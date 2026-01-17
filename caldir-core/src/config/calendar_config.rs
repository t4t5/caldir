//! Per-calendar local configuration.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CalDirError, CalDirResult};
use crate::remote::Remote;

/// Configuration stored in each calendar's .caldir/config.toml
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct CalendarConfig {
    pub remote: Option<Remote>,
}

impl CalendarConfig {
    /// Load config from .caldir/config.toml
    pub fn load(calendar_dir: &Path) -> CalDirResult<Self> {
        let path = calendar_dir.join(".caldir/config.toml");

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: CalendarConfig =
                toml::from_str(&content).map_err(|e| CalDirError::Config(e.to_string()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to .caldir/config.toml
    pub fn save(&self, calendar_dir: &Path) -> CalDirResult<()> {
        let dir = calendar_dir.join(".caldir");
        std::fs::create_dir_all(&dir)?;

        let path = dir.join("config.toml");

        let content =
            toml::to_string_pretty(self).map_err(|e| CalDirError::Config(e.to_string()))?;

        std::fs::write(&path, content)?;

        Ok(())
    }
}
