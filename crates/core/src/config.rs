use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{provider::ProviderSelection, Agent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    LocalEcho,
    RemoteOpenAiCompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub supports_tools: bool,
}

impl ModelConfig {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            supports_tools: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub label: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub models: BTreeMap<String, ModelConfig>,
}

impl ProviderConfig {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: ProviderKind::LocalEcho,
            api_base: None,
            api_key_env: None,
            default_model: None,
            models: BTreeMap::new(),
        }
    }

    pub fn has_model(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub app_name: String,
    #[serde(default)]
    pub default_agent: String,
    #[serde(default)]
    pub desktop_targets: Vec<String>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    #[serde(default)]
    pub agents: BTreeMap<String, Agent>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        let build = Agent::build();
        let plan = Agent::plan();
        let mut agents = BTreeMap::new();
        agents.insert(build.name.clone(), build);
        agents.insert(plan.name.clone(), plan);

        let mut providers = BTreeMap::new();
        let mut local = ProviderConfig::new("local", "Local / development provider");
        local.default_model = Some(String::from("echo"));
        local.models.insert(
            String::from("echo"),
            ModelConfig::new("echo", "Echo model for basic plumbing checks"),
        );
        local.models.insert(
            String::from("planner"),
            ModelConfig::new("planner", "Planning-flavored local echo model"),
        );
        providers.insert(local.id.clone(), local);

        let mut openai = ProviderConfig::new("openai", "OpenAI-compatible remote provider");
        openai.kind = ProviderKind::RemoteOpenAiCompatible;
        openai.api_base = Some(String::from("https://api.openai.com/v1"));
        openai.api_key_env = Some(String::from("OPENAI_API_KEY"));
        openai.default_model = Some(String::from("gpt-4.1-mini"));
        openai.models.insert(
            String::from("gpt-4.1-mini"),
            ModelConfig::new("gpt-4.1-mini", "Fast general coding model"),
        );
        openai.models.insert(
            String::from("gpt-4.1"),
            ModelConfig::new("gpt-4.1", "Higher-capability coding model"),
        );
        providers.insert(openai.id.clone(), openai);

        Self {
            app_name: String::from("Rovdex"),
            default_agent: String::from("build"),
            desktop_targets: vec![
                String::from("macos"),
                String::from("windows"),
                String::from("linux"),
            ],
            providers,
            agents,
        }
    }
}

impl WorkspaceConfig {
    pub fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.get(name)
    }

    pub fn provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    pub fn default_provider(&self) -> &ProviderConfig {
        self.providers
            .values()
            .next()
            .expect("workspace config must contain at least one provider")
    }

    pub fn default_agent(&self) -> &Agent {
        self.agent(&self.default_agent)
            .or_else(|| self.agents.values().next())
            .expect("workspace config must contain at least one agent")
    }

    pub fn resolve_provider_selection(
        &self,
        agent: &Agent,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<ProviderSelection> {
        let agent_selection = agent.model.as_deref().and_then(parse_model_reference);

        if let (Some(provider_id), Some(model_id)) = (provider_override, model_override) {
            let provider = self
                .provider(provider_id)
                .ok_or_else(|| anyhow!("unknown provider: {provider_id}"))?;
            if !provider.has_model(model_id) {
                return Err(anyhow!("provider {provider_id} does not expose model {model_id}"));
            }
            return Ok(ProviderSelection::new(provider_id, model_id));
        }

        if let Some(provider_id) = provider_override {
            let provider = self
                .provider(provider_id)
                .ok_or_else(|| anyhow!("unknown provider: {provider_id}"))?;
            let model_id = model_override
                .or(provider.default_model.as_deref())
                .ok_or_else(|| anyhow!("provider {provider_id} does not define a default model"))?;
            if !provider.has_model(model_id) {
                return Err(anyhow!("provider {provider_id} does not expose model {model_id}"));
            }
            return Ok(ProviderSelection::new(provider_id, model_id));
        }

        if let Some(model_id) = model_override {
            if let Some(provider) = self.providers.values().find(|provider| provider.has_model(model_id)) {
                return Ok(ProviderSelection::new(provider.id.clone(), model_id));
            }
            return Err(anyhow!("no provider exposes model {model_id}"));
        }

        if let Some((provider_id, model_id)) = agent_selection {
            let provider = self
                .provider(provider_id)
                .ok_or_else(|| anyhow!("unknown provider: {provider_id}"))?;
            if !provider.has_model(model_id) {
                return Err(anyhow!("provider {provider_id} does not expose model {model_id}"));
            }
            return Ok(ProviderSelection::new(provider_id, model_id));
        }

        let provider = self.default_provider();
        let model_id = provider
            .default_model
            .as_deref()
            .ok_or_else(|| anyhow!("default provider {} is missing a default model", provider.id))?;
        Ok(ProviderSelection::new(provider.id.clone(), model_id))
    }
}

fn parse_model_reference(value: &str) -> Option<(&str, &str)> {
    value.split_once('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_provider_selection() {
        let config = WorkspaceConfig::default();
        let selection = config
            .resolve_provider_selection(config.default_agent(), None, None)
            .expect("selection");

        assert_eq!(selection.provider_id, "local");
        assert_eq!(selection.model_id, "echo");
    }

    #[test]
    fn resolves_model_without_explicit_provider() {
        let config = WorkspaceConfig::default();
        let selection = config
            .resolve_provider_selection(config.default_agent(), None, Some("planner"))
            .expect("selection");

        assert_eq!(selection.provider_id, "local");
        assert_eq!(selection.model_id, "planner");
    }

    #[test]
    fn resolves_explicit_remote_provider_selection() {
        let config = WorkspaceConfig::default();
        let selection = config
            .resolve_provider_selection(config.default_agent(), Some("openai"), Some("gpt-4.1-mini"))
            .expect("selection");

        assert_eq!(selection.provider_id, "openai");
        assert_eq!(selection.model_id, "gpt-4.1-mini");
    }
}