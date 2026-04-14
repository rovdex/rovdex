use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{AppPaths, DesktopPlatform};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuthProvider {
    GitHubCopilot,
}

impl AuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GitHubCopilot => "github-copilot",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "copilot" | "github-copilot" | "github_copilot" => Ok(Self::GitHubCopilot),
            other => Err(anyhow!("unsupported auth provider: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthRecord {
    pub provider: AuthProvider,
    pub token: String,
    pub source: String,
    pub saved_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct AuthFile {
    #[serde(default)]
    records: BTreeMap<AuthProvider, AuthRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthStatus {
    pub provider: AuthProvider,
    pub source: Option<String>,
    pub stored: bool,
    pub auth_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenDiscovery {
    pub token: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotExchange {
    pub bearer_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AuthStore {
    path: PathBuf,
}

impl AuthStore {
    pub fn for_app(app_name: &str) -> Result<Self> {
        let paths = AppPaths::discover(app_name)?;
        Ok(Self::new(paths.config_dir_path().join("auth.json")))
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self, provider: AuthProvider) -> Result<AuthStatus> {
        let file = self.read_file()?;
        let source = file.records.get(&provider).map(|record| record.source.clone());
        let stored = source.is_some();
        Ok(AuthStatus {
            provider,
            source,
            stored,
            auth_file: self.path.display().to_string(),
        })
    }

    pub fn load(&self, provider: AuthProvider) -> Result<Option<AuthRecord>> {
        let file = self.read_file()?;
        Ok(file.records.get(&provider).cloned())
    }

    pub fn save(&self, provider: AuthProvider, token: String, source: String) -> Result<AuthRecord> {
        let mut file = self.read_file()?;
        let record = AuthRecord {
            provider: provider.clone(),
            token,
            source,
            saved_at_unix: unix_timestamp()?,
        };
        file.records.insert(provider, record.clone());
        self.write_file(&file)?;
        Ok(record)
    }

    pub fn delete(&self, provider: AuthProvider) -> Result<bool> {
        let mut file = self.read_file()?;
        let removed = file.records.remove(&provider).is_some();
        if removed {
            self.write_file(&file)?;
        }
        Ok(removed)
    }

    fn read_file(&self) -> Result<AuthFile> {
        if !self.path.exists() {
            return Ok(AuthFile::default());
        }

        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn write_file(&self, file: &AuthFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_vec_pretty(file)?)?;
        Ok(())
    }
}

pub fn discover_github_token() -> Result<TokenDiscovery> {
    if let Some(token) = std::env::var_os("GITHUB_TOKEN") {
        let token = token.to_string_lossy().trim().to_string();
        if !token.is_empty() {
            return Ok(TokenDiscovery {
                token,
                source: String::from("env:GITHUB_TOKEN"),
            });
        }
    }

    let platform = DesktopPlatform::current();
    let config_dir = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else if matches!(platform, DesktopPlatform::Windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData").join("Local"))
            })
            .ok_or_else(|| anyhow!("unable to determine LOCALAPPDATA for Windows token discovery"))?
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config"))
            .ok_or_else(|| anyhow!("unable to determine HOME for token discovery"))?
    };

    for path in [
        config_dir.join("github-copilot").join("hosts.json"),
        config_dir.join("github-copilot").join("apps.json"),
    ] {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(token) = parse_github_copilot_token(&content) {
            return Ok(TokenDiscovery {
                token,
                source: path.display().to_string(),
            });
        }
    }

    Err(anyhow!("GitHub token not found in standard locations"))
}

pub fn exchange_github_token_for_copilot(github_token: &str) -> Result<CopilotExchange> {
    let response = Client::new()
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("Token {github_token}"))
        .header("User-Agent", "Rovdex/0.1")
        .send()?
        .error_for_status()?;

    #[derive(Debug, Deserialize)]
    struct CopilotTokenResponse {
        token: String,
        #[serde(default)]
        expires_at: Option<i64>,
    }

    let body: CopilotTokenResponse = response.json()?;
    Ok(CopilotExchange {
        bearer_token: body.token,
        expires_at: body.expires_at,
    })
}

fn parse_github_copilot_token(content: &str) -> Option<String> {
    let config: serde_json::Value = serde_json::from_str(content).ok()?;
    let object = config.as_object()?;
    for (key, value) in object {
        if !key.contains("github.com") {
            continue;
        }
        if let Some(token) = value.get("oauth_token").and_then(serde_json::Value::as_str) {
            if !token.trim().is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn unix_timestamp() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("system time error: {error}"))?
        .as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hosts_json_token() {
        let content = r#"{
          "github.com": {
            "oauth_token": "gho_test_token"
          }
        }"#;

        let token = parse_github_copilot_token(content).expect("token");
        assert_eq!(token, "gho_test_token");
    }

    #[test]
    fn auth_store_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rovdex-auth-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let store = AuthStore::new(dir.join("auth.json"));
        let record = store
            .save(
                AuthProvider::GitHubCopilot,
                String::from("gho_test"),
                String::from("test"),
            )
            .expect("save");
        assert_eq!(record.source, "test");
        assert!(store
            .load(AuthProvider::GitHubCopilot)
            .expect("load")
            .is_some());
        let removed = store.delete(AuthProvider::GitHubCopilot).expect("delete");
        assert!(removed);
        let _ = fs::remove_dir_all(dir);
    }
}
