use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{Config, Context, Message, ProviderConfig, ProviderKind, Role, ToolSpec, WorkspaceConfig};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: Value,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, input: impl Into<Value>) -> Self {
        Self {
            name: name.into(),
            input: input.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub context: Context,
    pub selection: ProviderSelection,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub supports_tools: bool,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            supports_tools: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSelection {
    pub provider_id: String,
    pub model_id: String,
}

impl ProviderSelection {
    pub fn new(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub id: String,
    pub label: String,
    pub models: Vec<ModelInfo>,
}

pub struct RouterProvider {
    backends: BTreeMap<String, Arc<dyn Provider>>,
}

impl RouterProvider {
    pub fn new() -> Self {
        Self {
            backends: BTreeMap::new(),
        }
    }

    pub fn with_provider(mut self, provider_id: impl Into<String>, provider: impl Provider + 'static) -> Self {
        self.backends.insert(provider_id.into(), Arc::new(provider));
        self
    }

    pub fn from_config(config: &WorkspaceConfig) -> Self {
        let mut router = Self::new();
        for provider in config.providers.values() {
            match provider.kind {
                ProviderKind::LocalEcho => {
                    router = router.with_provider(provider.id.clone(), EchoProvider);
                }
                ProviderKind::RemoteOpenAiCompatible => {
                    router.backends.insert(
                        provider.id.clone(),
                        Arc::new(OpenAiCompatibleProvider::from_config(provider.clone())),
                    );
                }
            }
        }
        router
    }
}

impl Default for RouterProvider {
    fn default() -> Self {
        Self::new().with_provider("local", EchoProvider)
    }
}

impl Provider for RouterProvider {
    fn name(&self) -> &'static str {
        "router"
    }

    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let backend = self
            .backends
            .get(&request.selection.provider_id)
            .ok_or_else(|| anyhow!("unknown provider backend: {}", request.selection.provider_id))?;
        backend.complete(request)
    }
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
        "local"
    }

    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let prompt = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::User)
            .map(|message| message.content.clone())
            .unwrap_or_default();

        let prefix = match request.selection.model_id.as_str() {
            "planner" => "Planner",
            _ => "Echo",
        };

        Ok(ProviderResponse::final_message(format!("{prefix}: {prompt}")))
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

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    config: ProviderConfig,
    client: Client,
}

impl OpenAiCompatibleProvider {
    pub fn from_config(config: ProviderConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    fn api_base(&self) -> Result<&str> {
        self.config
            .api_base
            .as_deref()
            .ok_or_else(|| anyhow!("provider {} is missing api_base", self.config.id))
    }

    fn api_key(&self) -> Result<String> {
        let env_name = self
            .config
            .api_key_env
            .as_deref()
            .ok_or_else(|| anyhow!("provider {} is missing api_key_env", self.config.id))?;
        std::env::var(env_name)
            .map_err(|_| anyhow!("environment variable {env_name} is required for provider {}", self.config.id))
    }
}

impl Provider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str {
        "openai-compatible"
    }

    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let url = format!("{}/chat/completions", self.api_base()?.trim_end_matches('/'));
        let api_key = self.api_key()?;
        let payload = build_openai_payload(&request);
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&payload)
            .send()?
            .error_for_status()?;
        let body: OpenAiChatCompletionResponse = response.json()?;
        parse_openai_response(body)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
}

fn build_openai_payload(request: &ProviderRequest) -> Value {
    json!({
        "model": request.selection.model_id,
        "messages": request.messages.iter().map(|message| {
            json!({
                "role": openai_role(&message.role),
                "content": message.content,
            })
        }).collect::<Vec<_>>(),
    })
}

fn parse_openai_response(body: OpenAiChatCompletionResponse) -> Result<ProviderResponse> {
    let content = body
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .ok_or_else(|| anyhow!("openai-compatible response did not include assistant content"))?;

    Ok(ProviderResponse::final_message(content))
}

fn openai_role(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openai_payload_from_request() {
        let request = ProviderRequest {
            context: Context::from_path(std::env::current_dir().expect("cwd")).expect("context"),
            selection: ProviderSelection::new("openai", "gpt-4.1-mini"),
            messages: vec![
                Message::system("system prompt"),
                Message::user("hello"),
            ],
            tools: Vec::new(),
        };

        let payload = build_openai_payload(&request);
        assert_eq!(payload["model"], "gpt-4.1-mini");
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][1]["content"], "hello");
    }

    #[test]
    fn parses_openai_response_content() {
        let response = parse_openai_response(OpenAiChatCompletionResponse {
            choices: vec![OpenAiChoice {
                message: OpenAiMessage {
                    content: Some(String::from("remote answer")),
                },
            }],
        })
        .expect("provider response");

        assert_eq!(response.content, "remote answer");
    }
}
