use anyhow::{anyhow, Result};

use crate::{
    tools::{CurrentDirectoryTool, GitStatusTool, ListDirectoryTool, ReadFileTool},
    Context, EchoProvider, Message, Provider, ProviderRequest, Task, ToolRegistry,
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
    max_iterations: usize,
}

impl<P: Provider> Engine<P> {
    pub fn new(provider: P, tools: ToolRegistry) -> Self {
        Self {
            provider,
            tools,
            max_iterations: 8,
        }
    }

    pub fn with_standard_tools(provider: P) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(CurrentDirectoryTool);
        tools.register(ListDirectoryTool);
        tools.register(ReadFileTool);
        tools.register(GitStatusTool);
        Self::new(provider, tools)
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub fn run(&self, context: Context, task: Task) -> Result<RunResult> {
        let mut messages = vec![
            Message::system(build_system_prompt(&context)),
            Message::user(task.prompt.clone()),
        ];
        let mut final_message = String::new();

        for iteration in 0..self.max_iterations {
            let response = self.provider.complete(ProviderRequest {
                context: context.clone(),
                messages: messages.clone(),
                tools: self.tools.specs(),
            })?;

            if !response.content.is_empty() {
                final_message = response.content.clone();
                messages.push(Message::assistant(response.content));
            }

            if response.tool_calls.is_empty() {
                return Ok(RunResult {
                    task_id: task.id,
                    final_message,
                    iterations: iteration + 1,
                    messages,
                });
            }

            for call in response.tool_calls {
                let tool_result = self.tools.call(&call.name, &context, &call.input)?;
                messages.push(Message::tool(call.name, tool_result.output));
            }
        }

        Err(anyhow!(
            "provider did not finish within {} iterations",
            self.max_iterations
        ))
    }
}

impl Engine<EchoProvider> {
    pub fn echo() -> Self {
        Self::with_standard_tools(EchoProvider)
    }
}

fn build_system_prompt(context: &Context) -> String {
    let repository_root = context
        .repository_root
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());

    format!(
        "You are Rovdex, a coding agent. Use tools when they help.\nworkspace: {}\nrepository_root: {}\n",
        context.cwd.display(), repository_root
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
