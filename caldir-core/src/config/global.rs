//! Global caldir configuration.

use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{CalDirError, CalDirResult};

static DEFAULT_CALDIR_PATH: &str = "~/calendar";

fn default_caldir_path() -> PathBuf {
    PathBuf::from(DEFAULT_CALDIR_PATH)
}

/// Global configuration at ~/.config/caldir/config.toml
///
/// Calendar-specific configuration (provider, account, etc.) is stored
/// in each calendar's .caldir/config.toml file instead.
#[derive(Deserialize, Clone)]
pub struct GlobalConfig {
    #[serde(default = "default_caldir_path")]
    pub calendar_dir: PathBuf,

    pub default_calendar: Option<String>,
}

impl GlobalConfig {
    pub fn config_path() -> CalDirResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine config directory".into()))?
            .join("caldir");

        Ok(config_dir.join("config.toml"))
    }
}
