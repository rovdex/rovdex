use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Agent, Context, Message, ProviderSelection, Role, ToolCall};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_root: Option<String>,
    pub agent: Agent,
    pub provider: ProviderSelection,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
}

impl Session {
    pub fn new(
        id: impl Into<String>,
        context: &Context,
        agent: Agent,
        provider: ProviderSelection,
    ) -> Self {
        Self {
            id: id.into(),
            cwd: context.cwd.display().to_string(),
            repository_root: context
                .repository_root
                .as_ref()
                .map(|path| path.display().to_string()),
            agent,
            provider,
            messages: Vec::new(),
        }
    }

    pub fn push(&mut self, message: SessionMessage) {
        self.messages.push(message);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub role: Role,
    #[serde(default)]
    pub parts: Vec<MessagePart>,
}

impl SessionMessage {
    pub fn from_text(id: impl Into<String>, role: Role, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            parts: vec![MessagePart::Text { text: text.into() }],
        }
    }

    pub fn from_message(id: impl Into<String>, message: Message) -> Self {
        let mut parts = Vec::new();
        if !message.content.is_empty() {
            parts.push(MessagePart::Text {
                text: message.content,
            });
        }
        Self {
            id: id.into(),
            role: message.role,
            parts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePart {
    Text {
        text: String,
    },
    ToolCall {
        tool: String,
        input: Value,
    },
    ToolResult {
        tool: String,
        output: Value,
    },
    State {
        key: String,
        value: Value,
    },
}

impl MessagePart {
    pub fn tool_call(call: &ToolCall) -> Self {
        Self::ToolCall {
            tool: call.name.clone(),
            input: call.input.clone(),
        }
    }

    pub fn tool_result(tool: impl Into<String>, output: impl Into<Value>) -> Self {
        Self::ToolResult {
            tool: tool.into(),
            output: output.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    SessionStarted {
        session_id: String,
        agent: String,
        provider: String,
        model: String,
    },
    MessageRecorded {
        session_id: String,
        message_id: String,
        role: Role,
    },
    ToolCalled {
        session_id: String,
        message_id: String,
        tool: String,
    },
    PermissionRequired {
        session_id: String,
        message_id: String,
        tool: String,
        scope: String,
        target: String,
    },
    ToolDenied {
        session_id: String,
        message_id: String,
        tool: String,
        scope: String,
        target: String,
    },
    ToolCompleted {
        session_id: String,
        message_id: String,
        tool: String,
    },
    SessionFinished {
        session_id: String,
        iterations: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRun {
    pub session: Session,
    pub events: Vec<SessionEvent>,
    pub final_message: String,
    pub iterations: usize,
}