use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopPlatform {
    MacOS,
    Windows,
    Linux,
    Unknown(String),
}

impl DesktopPlatform {
    pub fn current() -> Self {
        match env::consts::OS {
            "macos" => Self::MacOS,
            "windows" => Self::Windows,
            "linux" => Self::Linux,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::MacOS => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPaths {
    pub app_name: String,
    pub platform: DesktopPlatform,
    pub home_dir: String,
    pub data_dir: String,
    pub config_dir: String,
    pub cache_dir: String,
}

impl AppPaths {
    pub fn discover(app_name: impl Into<String>) -> Result<Self> {
        let app_name = app_name.into();
        let platform = DesktopPlatform::current();
        let home = home_dir()?;

        let (data_dir, config_dir, cache_dir) = match &platform {
            DesktopPlatform::MacOS => (
                home.join("Library/Application Support").join(&app_name),
                home.join("Library/Application Support").join(&app_name),
                home.join("Library/Caches").join(&app_name),
            ),
            DesktopPlatform::Windows => {
                let app_data = env_path("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming"));
                let local_app_data =
                    env_path("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData/Local"));
                (
                    app_data.join(&app_name),
                    app_data.join(&app_name),
                    local_app_data.join(&app_name).join("Cache"),
                )
            }
            DesktopPlatform::Linux | DesktopPlatform::Unknown(_) => {
                let data = env_path("XDG_DATA_HOME")
                    .unwrap_or_else(|| home.join(".local/share"))
                    .join(&app_name);
                let config = env_path("XDG_CONFIG_HOME")
                    .unwrap_or_else(|| home.join(".config"))
                    .join(&app_name);
                let cache = env_path("XDG_CACHE_HOME")
                    .unwrap_or_else(|| home.join(".cache"))
                    .join(&app_name);
                (data, config, cache)
            }
        };

        Ok(Self {
            app_name,
            platform,
            home_dir: home.display().to_string(),
            data_dir: data_dir.display().to_string(),
            config_dir: config_dir.display().to_string(),
            cache_dir: cache_dir.display().to_string(),
        })
    }

    pub fn data_dir_path(&self) -> &Path {
        Path::new(&self.data_dir)
    }

    pub fn config_dir_path(&self) -> &Path {
        Path::new(&self.config_dir)
    }

    pub fn cache_dir_path(&self) -> &Path {
        Path::new(&self.cache_dir)
    }
}

fn home_dir() -> Result<PathBuf> {
    env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .ok_or_else(|| anyhow!("unable to determine user home directory"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_platform_paths() {
        let paths = AppPaths::discover("Rovdex").expect("paths");
        assert_eq!(paths.app_name, "Rovdex");
        assert!(!paths.data_dir.is_empty());
        assert!(!paths.config_dir.is_empty());
        assert!(!paths.cache_dir.is_empty());
    }
}
