use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{AppPaths, Context, SessionRun};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSession {
    pub id: String,
    pub created_at_unix: u64,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_root: Option<String>,
    pub provider: String,
    pub model: String,
    pub agent: String,
    pub iterations: usize,
    pub final_message: String,
    pub run: SessionRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub created_at_unix: u64,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_root: Option<String>,
    pub provider: String,
    pub model: String,
    pub agent: String,
    pub iterations: usize,
    pub final_message_preview: String,
}

pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn for_context(context: &Context) -> Self {
        let base = context
            .repository_root
            .clone()
            .unwrap_or_else(|| context.cwd.clone());
        Self::new(base.join(".rovdex").join("sessions"))
    }

    pub fn for_app(app_name: &str) -> Result<Self> {
        let paths = AppPaths::discover(app_name)?;
        Ok(Self::new(paths.data_dir_path().join("sessions")))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn save_run(&self, run: &SessionRun) -> Result<StoredSession> {
        fs::create_dir_all(&self.root)?;

        let created_at_unix = unix_timestamp()?;
        let unique_suffix = unix_timestamp_millis()?;
        let id = if run.session.id == "session" || run.session.id.is_empty() {
            format!("session-{unique_suffix}")
        } else {
            run.session.id.clone()
        };

        let stored = StoredSession {
            id: id.clone(),
            created_at_unix,
            cwd: run.session.cwd.clone(),
            repository_root: run.session.repository_root.clone(),
            provider: run.session.provider.provider_id.clone(),
            model: run.session.provider.model_id.clone(),
            agent: run.session.agent.name.clone(),
            iterations: run.iterations,
            final_message: run.final_message.clone(),
            run: run.clone(),
        };

        let path = self.root.join(format!("{id}.json"));
        fs::write(&path, serde_json::to_vec_pretty(&stored)?)?;
        Ok(stored)
    }

    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(stored) = serde_json::from_str::<StoredSession>(&content) else {
                continue;
            };
            sessions.push(SessionSummary {
                id: stored.id,
                created_at_unix: stored.created_at_unix,
                cwd: stored.cwd,
                repository_root: stored.repository_root,
                provider: stored.provider,
                model: stored.model,
                agent: stored.agent,
                iterations: stored.iterations,
                final_message_preview: preview_text(&stored.final_message, 96),
            });
        }

        sessions.sort_by(|a, b| b.created_at_unix.cmp(&a.created_at_unix).then_with(|| a.id.cmp(&b.id)));
        Ok(sessions)
    }

    pub fn latest(&self) -> Result<Option<StoredSession>> {
        let summary = self.list()?.into_iter().next();
        match summary {
            Some(summary) => self.load(&summary.id).map(Some),
            None => Ok(None),
        }
    }

    pub fn load(&self, id: &str) -> Result<StoredSession> {
        let path = self.root.join(format!("{id}.json"));
        let content = fs::read_to_string(&path)
            .map_err(|error| anyhow!("failed to read {}: {error}", path.display()))?;
        Ok(serde_json::from_str(&content)?)
    }
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }

    compact.chars().take(max_chars.saturating_sub(1)).collect::<String>() + "…"
}

fn unix_timestamp() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("system time error: {error}"))?
        .as_secs())
}

fn unix_timestamp_millis() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("system time error: {error}"))?
        .as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Agent, AgentMode, Context, ProviderSelection, Session, SessionRun};

    fn temp_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rovdex-session-store-{unique}"));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn saves_and_loads_sessions() {
        let root = temp_root();
        let store = SessionStore::new(&root);
        let context = Context::from_path(&root).expect("context");
        let session = Session::new(
            "session",
            &context,
            Agent::new("build", "Build agent", AgentMode::Primary),
            ProviderSelection::new("local", "echo"),
        );
        let run = SessionRun {
            session,
            events: Vec::new(),
            final_message: String::from("done"),
            iterations: 1,
        };

        let stored = store.save_run(&run).expect("save");
        let list = store.list().expect("list");
        let loaded = store.load(&stored.id).expect("load");

        assert_eq!(list.len(), 1);
        assert_eq!(loaded.final_message, "done");

        let _ = fs::remove_dir_all(root);
    }
}
