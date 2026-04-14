use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    pub fn tool(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self::tool_with_id(name, Option::<String>::None, content)
    }

    pub fn tool_with_id(
        name: impl Into<String>,
        tool_call_id: impl Into<Option<String>>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            name: Some(name.into()),
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }

    fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            name: None,
            tool_call_id: None,
            content: content.into(),
        }
    }
}
