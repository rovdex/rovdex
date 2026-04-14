use anyhow::{anyhow, Result};
use serde_json::json;

use crate::{
    Agent, PermissionAction, ProviderSelection, WorkspaceConfig,
    tools::{
        BashTool, CurrentDirectoryTool, EditFileTool, GitStatusTool, GlobTool, GrepTool,
        ListDirectoryTool, ReadFileTool, WorkspaceMapTool, WriteFileTool,
    },
    Context, EchoProvider, Message, Provider, ProviderRequest, Session, SessionEvent,
    SessionMessage, SessionRun, Task, ToolRegistry, WorkspaceMap,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub task_id: String,
    pub final_message: String,
    pub iterations: usize,
    pub messages: Vec<Message>,
}

pub struct Engine<P: Provider> {
    provider: P,
    tools: ToolRegistry,
    config: WorkspaceConfig,
    max_iterations: usize,
}

impl<P: Provider> Engine<P> {
    pub fn new(provider: P, tools: ToolRegistry) -> Self {
        Self {
            provider,
            tools,
            config: WorkspaceConfig::default(),
            max_iterations: 8,
        }
    }

    pub fn with_config(mut self, config: WorkspaceConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_standard_tools(provider: P) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(BashTool);
        tools.register(CurrentDirectoryTool);
        tools.register(EditFileTool);
        tools.register(ListDirectoryTool);
        tools.register(GlobTool);
        tools.register(GrepTool);
        tools.register(ReadFileTool);
        tools.register(WriteFileTool);
        tools.register(GitStatusTool);
        tools.register(WorkspaceMapTool);
        Self::new(provider, tools)
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub fn config(&self) -> &WorkspaceConfig {
        &self.config
    }

    pub fn run_with_agent(
        &self,
        context: Context,
        task: Task,
        agent_name: Option<&str>,
    ) -> Result<SessionRun> {
        self.run_with_selection(context, task, agent_name, None, None)
    }

    pub fn run_with_selection(
        &self,
        context: Context,
        task: Task,
        agent_name: Option<&str>,
        provider_name: Option<&str>,
        model_name: Option<&str>,
    ) -> Result<SessionRun> {
        let agent = agent_name
            .and_then(|name| self.config.agent(name).cloned())
            .unwrap_or_else(|| self.config.default_agent().clone());
        let selection = self
            .config
            .resolve_provider_selection(&agent, provider_name, model_name)?;

        self.run_session(context, task, agent, selection)
    }

    pub fn run_session(
        &self,
        context: Context,
        task: Task,
        agent: Agent,
        selection: ProviderSelection,
    ) -> Result<SessionRun> {
        let mut events = vec![SessionEvent::SessionStarted {
            session_id: task.id.clone(),
            agent: agent.name.clone(),
            provider: selection.provider_id.clone(),
            model: selection.model_id.clone(),
        }];
        let mut session = Session::new(task.id.clone(), &context, agent.clone(), selection.clone());

        let system_message = Message::system(build_system_prompt(
            &context,
            &agent,
            &selection,
            &self.config,
        ));
        session.push(SessionMessage::from_message(
            format!("{}-system", task.id),
            system_message.clone(),
        ));
        events.push(SessionEvent::MessageRecorded {
            session_id: task.id.clone(),
            message_id: format!("{}-system", task.id),
            role: system_message.role.clone(),
        });

        let user_message = Message::user(task.prompt.clone());
        session.push(SessionMessage::from_message(
            format!("{}-user-0", task.id),
            user_message.clone(),
        ));
        events.push(SessionEvent::MessageRecorded {
            session_id: task.id.clone(),
            message_id: format!("{}-user-0", task.id),
            role: user_message.role.clone(),
        });

        let mut messages = vec![system_message, user_message];
        let mut final_message = String::new();

        for iteration in 0..self.max_iterations {
            let response = self.provider.complete(ProviderRequest {
                context: context.clone(),
                selection: selection.clone(),
                messages: messages.clone(),
                tools: self
                    .tools
                    .specs()
                    .into_iter()
                    .filter(|tool| agent.is_tool_enabled(&tool.name))
                    .collect(),
            })?;

            if !response.content.is_empty() {
                final_message = response.content.clone();
                let assistant_message = Message::assistant(response.content);
                let assistant_id = format!("{}-assistant-{}", task.id, iteration);
                session.push(SessionMessage::from_message(
                    assistant_id.clone(),
                    assistant_message.clone(),
                ));
                events.push(SessionEvent::MessageRecorded {
                    session_id: task.id.clone(),
                    message_id: assistant_id,
                    role: assistant_message.role.clone(),
                });
                messages.push(assistant_message);
            }

            if response.tool_calls.is_empty() {
                events.push(SessionEvent::SessionFinished {
                    session_id: task.id.clone(),
                    iterations: iteration + 1,
                });
                return Ok(SessionRun {
                    session,
                    events,
                    final_message,
                    iterations: iteration + 1,
                });
            }

            for (tool_index, call) in response.tool_calls.into_iter().enumerate() {
                if !agent.is_tool_enabled(&call.name) {
                    return Err(anyhow!("tool {} is disabled for agent {}", call.name, agent.name));
                }

                let tool_message_id = format!("{}-tool-{}-{}", task.id, iteration, tool_index);
                events.push(SessionEvent::ToolCalled {
                    session_id: task.id.clone(),
                    message_id: tool_message_id.clone(),
                    tool: call.name.clone(),
                });

                let tool_result = if let Some(permission_request) =
                    agent.permission_for_tool(&call.name, &call.input)
                {
                    match agent.evaluate_permission(&permission_request) {
                        PermissionAction::Allow => self.tools.call(&call.name, &context, &call.input)?,
                        PermissionAction::Ask => {
                            events.push(SessionEvent::PermissionRequired {
                                session_id: task.id.clone(),
                                message_id: tool_message_id.clone(),
                                tool: call.name.clone(),
                                scope: format!("{:?}", permission_request.scope),
                                target: permission_request.target.clone(),
                            });
                            crate::ToolResult::new(json!({
                                "status": "permission_required",
                                "tool": call.name,
                                "scope": format!("{:?}", permission_request.scope),
                                "target": permission_request.target,
                                "message": "This tool requires user approval before execution.",
                            }))
                        }
                        PermissionAction::Deny => {
                            events.push(SessionEvent::ToolDenied {
                                session_id: task.id.clone(),
                                message_id: tool_message_id.clone(),
                                tool: call.name.clone(),
                                scope: format!("{:?}", permission_request.scope),
                                target: permission_request.target.clone(),
                            });
                            crate::ToolResult::new(json!({
                                "status": "permission_denied",
                                "tool": call.name,
                                "scope": format!("{:?}", permission_request.scope),
                                "target": permission_request.target,
                                "message": "This tool is denied by the current agent permission rules.",
                            }))
                        }
                    }
                } else {
                    self.tools.call(&call.name, &context, &call.input)?
                };
                session.push(SessionMessage {
                    id: tool_message_id.clone(),
                    role: crate::Role::Tool,
                    parts: vec![
                        crate::MessagePart::tool_call(&call),
                        crate::MessagePart::tool_result(call.name.clone(), tool_result.output.clone()),
                    ],
                });
                events.push(SessionEvent::ToolCompleted {
                    session_id: task.id.clone(),
                    message_id: tool_message_id,
                    tool: call.name.clone(),
                });
                messages.push(Message::tool_with_id(
                    call.name,
                    call.id,
                    tool_result.render(),
                ));
            }
        }

        Err(anyhow!(
            "provider did not finish within {} iterations",
            self.max_iterations
        ))
    }

    pub fn run(&self, context: Context, task: Task) -> Result<RunResult> {
        let session_run = self.run_with_agent(context, task, None)?;
        let mut messages = Vec::new();
        for session_message in &session_run.session.messages {
            let mut text = String::new();
            for part in &session_message.parts {
                match part {
                    crate::MessagePart::Text { text: value } => {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(value);
                    }
                    crate::MessagePart::ToolResult { tool, output } => {
                        let rendered = match output {
                            serde_json::Value::String(value) => value.clone(),
                            other => serde_json::to_string_pretty(other)
                                .unwrap_or_else(|_| other.to_string()),
                        };
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&rendered);
                        messages.push(Message::tool(tool.clone(), rendered));
                    }
                    crate::MessagePart::ToolCall { .. } | crate::MessagePart::State { .. } => {}
                }
            }

            if !text.is_empty() && session_message.role != crate::Role::Tool {
                messages.push(Message {
                    role: session_message.role.clone(),
                    name: None,
                    tool_call_id: None,
                    content: text,
                });
            }
        }

        Ok(RunResult {
            task_id: session_run.session.id,
            final_message: session_run.final_message,
            iterations: session_run.iterations,
            messages,
        })
    }
}

impl Engine<EchoProvider> {
    pub fn echo() -> Self {
        Self::with_standard_tools(EchoProvider)
    }
}

fn build_system_prompt(
    context: &Context,
    agent: &Agent,
    selection: &ProviderSelection,
    config: &WorkspaceConfig,
) -> String {
    let repository_root = context
        .repository_root
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());

    format!(
        "You are {}, a coding agent oriented toward Codex-style workflows.\nagent: {}\nagent_mode: {:?}\nprovider: {}\nmodel: {}\nworkspace: {}\nrepository_root: {}\ndesktop_targets: {}\n{}\nUse tools when they help, prefer repository-aware actions, and keep behavior portable across macOS, Windows, and Linux.\n",
        config.app_name,
        agent.name,
        agent.mode,
        selection.provider_id,
        selection.model_id,
        context.cwd.display(),
        repository_root,
        config.desktop_targets.join(", "),
        build_workspace_map_section(context),
    )
}

fn build_workspace_map_section(context: &Context) -> String {
    let root = context
        .repository_root
        .as_ref()
        .unwrap_or(&context.cwd);
    match WorkspaceMap::scan(root) {
        Ok(map) => map.render_markdown(),
        Err(error) => format!("Workspace map unavailable: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::{ProviderResponse, Role, ScriptedProvider, ToolCall};

    #[test]
    fn runs_tool_loop_and_finishes() {
        let provider = ScriptedProvider::new([
            ProviderResponse::with_tool_calls(
                "Inspecting workspace",
                vec![ToolCall::new("current_directory", "")],
            ),
            ProviderResponse::final_message("done"),
        ]);

        let engine = Engine::with_standard_tools(provider);
        let context = Context::from_path(std::env::current_dir().expect("cwd")).expect("context");
        let result = engine
            .run(context, Task::new("task-1", "Check the workspace"))
            .expect("run result");

        assert_eq!(result.final_message, "done");
        assert_eq!(result.iterations, 2);
        assert!(result
            .messages
            .iter()
            .any(|message| message.role == Role::Tool
                && message.name.as_deref() == Some("current_directory")));
    }

    #[test]
    fn session_run_records_structured_tool_output() {
        let provider = ScriptedProvider::new([
            ProviderResponse::with_tool_calls(
                "Inspecting workspace",
                vec![ToolCall::new("current_directory", json!({}))],
            ),
            ProviderResponse::final_message("done"),
        ]);

        let engine = Engine::with_standard_tools(provider);
        let context = Context::from_path(std::env::current_dir().expect("cwd")).expect("context");
        let result = engine
            .run_with_selection(
                context,
                Task::new("task-2", "Check the workspace"),
                Some("build"),
                Some("local"),
                Some("echo"),
            )
            .expect("session run");

        let tool_message = result
            .session
            .messages
            .iter()
            .find(|message| message.role == Role::Tool)
            .expect("tool message");

        assert!(tool_message.parts.iter().any(|part| matches!(
            part,
            crate::MessagePart::ToolResult { tool, output }
                if tool == "current_directory"
                    && output.get("cwd").is_some()
                    && output.get("repository_root").is_some()
        )));
        assert_eq!(result.session.provider.provider_id, "local");
        assert_eq!(result.session.provider.model_id, "echo");
    }

    #[test]
    fn plan_agent_denies_write_tool_calls() {
        let provider = ScriptedProvider::new([
            ProviderResponse::with_tool_calls(
                "Trying write",
                vec![ToolCall::new(
                    "write",
                    json!({
                        "path": "note.txt",
                        "content": "hello"
                    }),
                )],
            ),
            ProviderResponse::final_message("done"),
        ]);

        let engine = Engine::with_standard_tools(provider);
        let context = Context::from_path(std::env::current_dir().expect("cwd")).expect("context");
        let result = engine
            .run_with_selection(
                context,
                Task::new("task-3", "Try writing"),
                Some("plan"),
                Some("local"),
                Some("planner"),
            )
            .expect("session run");

        assert!(result.events.iter().any(|event| matches!(
            event,
            SessionEvent::ToolDenied { tool, .. } if tool == "write"
        )));

        let tool_message = result
            .session
            .messages
            .iter()
            .find(|message| message.role == Role::Tool)
            .expect("tool message");

        assert!(tool_message.parts.iter().any(|part| matches!(
            part,
            crate::MessagePart::ToolResult { output, .. }
                if output.get("status") == Some(&json!("permission_denied"))
        )));
    }

    #[test]
    fn build_agent_requires_approval_for_bash_tool_calls() {
        let provider = ScriptedProvider::new([
            ProviderResponse::with_tool_calls(
                "Trying bash",
                vec![ToolCall::new(
                    "bash",
                    json!({
                        "command": "pwd"
                    }),
                )],
            ),
            ProviderResponse::final_message("done"),
        ]);

        let engine = Engine::with_standard_tools(provider);
        let context = Context::from_path(std::env::current_dir().expect("cwd")).expect("context");
        let result = engine
            .run_with_selection(
                context,
                Task::new("task-4", "Try bash"),
                Some("build"),
                Some("local"),
                Some("echo"),
            )
            .expect("session run");

        assert!(result.events.iter().any(|event| matches!(
            event,
            SessionEvent::PermissionRequired { tool, .. } if tool == "bash"
        )));
    }
}
