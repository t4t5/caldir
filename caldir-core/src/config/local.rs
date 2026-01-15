//! Per-calendar local configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::CalDirResult;

/// Configuration stored in each calendar's .caldir/config.toml
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LocalConfig {
    pub remote: Option<RemoteConfig>,
}

/// Remote provider configuration (e.g., Google Calendar settings)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteConfig {
    pub provider: String,
    #[serde(flatten)]
    pub params: HashMap<String, toml::Value>,
}

impl LocalConfig {
    /// Load config from .caldir/config.toml
    pub fn load(calendar_dir: &Path) -> CalDirResult<Self> {
        let path = calendar_dir.join(".caldir/config.toml");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: LocalConfig = toml::from_str(&content)
                .map_err(|e| crate::error::CalDirError::Config(e.to_string()))?;
            Ok(config)
        } else {
            Ok(LocalConfig::default())
        }
    }

    /// Save config to .caldir/config.toml
    pub fn save(&self, calendar_dir: &Path) -> CalDirResult<()> {
        let dir = calendar_dir.join(".caldir");
        std::fs::create_dir_all(&dir)?;

        let path = dir.join("config.toml");
        let content =
            toml::to_string_pretty(self).map_err(|e| crate::error::CalDirError::Config(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
