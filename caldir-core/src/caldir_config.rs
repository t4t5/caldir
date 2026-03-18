//! Global caldir configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CalDirError, CalDirResult};
use crate::event::Reminder;

/// Time display format: 24-hour ("15:00") or 12-hour ("3:00pm").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeFormat {
    #[default]
    #[serde(rename = "24h")]
    H24,
    #[serde(rename = "12h")]
    H12,
}

static DEFAULT_CALDIR_PATH: &str = "~/caldir";

fn default_caldir_path() -> PathBuf {
    PathBuf::from(DEFAULT_CALDIR_PATH)
}

fn is_default_caldir_path(p: &PathBuf) -> bool {
    *p == default_caldir_path()
}

/// Global configuration at ~/.config/caldir/config.toml
///
/// Calendar-specific configuration (provider, account, etc.) is stored
/// in each calendar's .caldir/config.toml file instead.
#[derive(Serialize, Deserialize, Clone)]
pub struct CaldirConfig {
    #[serde(
        default = "default_caldir_path",
        skip_serializing_if = "is_default_caldir_path"
    )]
    pub calendar_dir: PathBuf,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_calendar: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_reminders: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "is_default_time_format")]
    pub time_format: TimeFormat,
}

fn is_default_time_format(f: &TimeFormat) -> bool {
    *f == TimeFormat::default()
}

impl CaldirConfig {
    pub fn config_path() -> CalDirResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine config directory".into()))?
            .join("caldir");

        Ok(config_dir.join("config.toml"))
    }

    /// Save the current config to ~/.config/caldir/config.toml
    pub fn save(&self) -> CalDirResult<()> {
        let config_path = Self::config_path()?;

        let content =
            toml::to_string_pretty(self).map_err(|e| CalDirError::Config(e.to_string()))?;

        std::fs::write(&config_path, content)
            .map_err(|e| CalDirError::Config(format!("Could not write config file: {e}")))?;

        Ok(())
    }

    /// Parse default_reminders strings into Reminder structs.
    pub fn parse_default_reminders(&self) -> CalDirResult<Option<Vec<Reminder>>> {
        let Some(ref strs) = self.default_reminders else {
            return Ok(None);
        };
        let reminders: Vec<Reminder> = strs
            .iter()
            .map(|s| Reminder::from_duration_str(s).map_err(CalDirError::Config))
            .collect::<CalDirResult<_>>()?;
        Ok(Some(reminders))
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

# Default reminders for new events (e.g. [\"10m\", \"1h\"]):
# default_reminders = [\"10m\", \"1h\"]

# Time display format: \"24h\" (default, e.g. 15:00) or \"12h\" (e.g. 3:00pm)
# time_format = \"12h\"
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
