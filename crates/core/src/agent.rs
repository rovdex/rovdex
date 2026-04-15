use std::collections::BTreeMap;

use glob::Pattern;
use serde_json::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    Primary,
    Subagent,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionScope {
    Read,
    Write,
    Bash,
    Web,
    Task,
    Question,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionAction {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub scope: PermissionScope,
    pub pattern: String,
    pub action: PermissionAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub scope: PermissionScope,
    pub target: String,
}

impl PermissionRule {
    pub fn allow(scope: PermissionScope, pattern: impl Into<String>) -> Self {
        Self {
            scope,
            pattern: pattern.into(),
            action: PermissionAction::Allow,
        }
    }

    pub fn ask(scope: PermissionScope, pattern: impl Into<String>) -> Self {
        Self {
            scope,
            pattern: pattern.into(),
            action: PermissionAction::Ask,
        }
    }

    pub fn deny(scope: PermissionScope, pattern: impl Into<String>) -> Self {
        Self {
            scope,
            pattern: pattern.into(),
            action: PermissionAction::Deny,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub description: String,
    pub mode: AgentMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub tools: BTreeMap<String, bool>,
    #[serde(default)]
    pub permissions: Vec<PermissionRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
}

impl Agent {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        mode: AgentMode,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            mode,
            model: None,
            prompt: None,
            tools: BTreeMap::new(),
            permissions: Vec::new(),
            max_steps: None,
        }
    }

    pub fn build() -> Self {
        let mut agent = Self::new(
            "build",
            "Default coding agent with repository tool access.",
            AgentMode::Primary,
        );
        agent.tools.extend([
            (String::from("bash"), true),
            (String::from("current_directory"), true),
            (String::from("edit"), true),
            (String::from("glob"), true),
            (String::from("grep"), true),
            (String::from("list_directory"), true),
            (String::from("read_file"), true),
            (String::from("workspace_map"), true),
            (String::from("write"), true),
            (String::from("git_status"), true),
        ]);
        agent.permissions.extend([
            PermissionRule::allow(PermissionScope::Read, "*"),
            PermissionRule::allow(PermissionScope::Write, "*"),
            PermissionRule::ask(PermissionScope::Bash, "*"),
        ]);
        agent.max_steps = Some(12);
        agent
    }

    pub fn plan() -> Self {
        let mut agent = Self::new(
            "plan",
            "Read-only planning agent for repository analysis.",
            AgentMode::Primary,
        );
        agent.model = Some(String::from("local/planner"));
        agent.tools.extend([
            (String::from("bash"), true),
            (String::from("current_directory"), true),
            (String::from("glob"), true),
            (String::from("grep"), true),
            (String::from("list_directory"), true),
            (String::from("read_file"), true),
            (String::from("workspace_map"), true),
            (String::from("git_status"), true),
        ]);
        agent.permissions.extend([
            PermissionRule::allow(PermissionScope::Read, "*"),
            PermissionRule::deny(PermissionScope::Write, "*"),
            PermissionRule::ask(PermissionScope::Bash, "*"),
        ]);
        agent.max_steps = Some(8);
        agent
    }

    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        self.tools.get(tool_name).copied().unwrap_or(true)
    }

    pub fn permission_for_tool(&self, tool_name: &str, input: &Value) -> Option<PermissionRequest> {
        let scope = match tool_name {
            "current_directory" | "list_directory" | "read_file" | "git_status" | "glob" | "grep" => {
                PermissionScope::Read
            }
            "write" | "edit" => PermissionScope::Write,
            "bash" => PermissionScope::Bash,
            _ => return None,
        };

        let target = match tool_name {
            "bash" => input
                .as_object()
                .and_then(|map| map.get("command"))
                .and_then(Value::as_str)
                .unwrap_or("*")
                .to_string(),
            "glob" => input
                .as_object()
                .and_then(|map| map.get("pattern"))
                .and_then(Value::as_str)
                .unwrap_or("*")
                .to_string(),
            _ => input
                .as_object()
                .and_then(|map| map.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("*")
                .to_string(),
        };

        Some(PermissionRequest { scope, target })
    }

    pub fn evaluate_permission(&self, request: &PermissionRequest) -> PermissionAction {
        self.permissions
            .iter()
            .rev()
            .find(|rule| {
                rule.scope == request.scope && permission_pattern_matches(&rule.pattern, &request.target)
            })
            .map(|rule| rule.action.clone())
            .unwrap_or(PermissionAction::Ask)
    }
}

fn permission_pattern_matches(pattern: &str, target: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    Pattern::new(pattern)
        .map(|glob| glob.matches(target))
        .unwrap_or_else(|_| pattern == target)
}
