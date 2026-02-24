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
pub struct CaldirConfig {
    #[serde(default = "default_caldir_path")]
    pub calendar_dir: PathBuf,

    pub default_calendar: Option<String>,
}

impl CaldirConfig {
    pub fn config_path() -> CalDirResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine config directory".into()))?
            .join("caldir");

        Ok(config_dir.join("config.toml"))
    }

    /// Create a default config file with all options commented out.
    pub fn create_default_config(path: &std::path::Path) -> CalDirResult<()> {
        let contents = format!(
            "\
# caldir configuration

# Where your calendars live:
# calendar_dir = \"{}\"

# Default calendar for new events:
# default_calendar = \"personal\"
",
            DEFAULT_CALDIR_PATH
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CalDirError::Config(format!("Could not create config directory: {e}"))
            })?;
        }

        std::fs::write(path, contents)
            .map_err(|e| CalDirError::Config(format!("Could not write config file: {e}")))?;

        Ok(())
    }
}
