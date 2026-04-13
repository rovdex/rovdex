use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub cwd: PathBuf,
    pub repository_root: Option<PathBuf>,
}

impl Context {
    pub fn from_current_dir() -> Result<Self> {
        Self::from_path(std::env::current_dir()?)
    }

    pub fn from_path(cwd: impl Into<PathBuf>) -> Result<Self> {
        let cwd = cwd.into();
        let repository_root = find_repository_root(&cwd);

        Ok(Self {
            cwd,
            repository_root,
        })
    }
}

fn find_repository_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}
