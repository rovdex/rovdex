use std::{collections::VecDeque, sync::Mutex};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{Context, Message, Role, ToolSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input: input.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub context: Context,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

impl ProviderResponse {
    pub fn final_message(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
        }
    }

    pub fn with_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content: content.into(),
            tool_calls,
        }
    }
}

pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;

    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

#[derive(Debug, Default)]
pub struct EchoProvider;

impl Provider for EchoProvider {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let prompt = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::User)
            .map(|message| message.content.clone())
            .unwrap_or_default();

        Ok(ProviderResponse::final_message(format!("Echo: {prompt}")))
    }
}

#[derive(Debug)]
pub struct ScriptedProvider {
    responses: Mutex<VecDeque<ProviderResponse>>,
}

impl ScriptedProvider {
    pub fn new(responses: impl IntoIterator<Item = ProviderResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

impl Default for ScriptedProvider {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Provider for ScriptedProvider {
    fn name(&self) -> &'static str {
        "scripted"
    }

    fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
        self.responses
            .lock()
            .expect("scripted provider mutex poisoned")
            .pop_front()
            .ok_or_else(|| anyhow!("scripted provider exhausted"))
    }
}
