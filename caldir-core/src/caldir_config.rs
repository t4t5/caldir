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

/// Global configuration at ~/.config/caldir/config.toml
///
/// Calendar-specific configuration (provider, account, etc.) is stored
/// in each calendar's .caldir/config.toml file instead.
#[derive(Serialize, Deserialize, Clone)]
pub struct CaldirConfig {
    #[serde(default = "default_caldir_path")]
    pub calendar_dir: PathBuf,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_calendar: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_reminders: Option<Vec<String>>,

    #[serde(default)]
    pub time_format: TimeFormat,
}

impl Default for CaldirConfig {
    fn default() -> Self {
        Self {
            calendar_dir: default_caldir_path(),
            default_calendar: None,
            default_reminders: None,
            time_format: TimeFormat::default(),
        }
    }
}

impl CaldirConfig {
    /// Caldir config directory:
    /// - Linux/BSD: `$XDG_CONFIG_HOME/caldir` or `~/.config/caldir`
    /// - macOS: `~/.config/caldir`
    /// - Windows: `%APPDATA%\caldir`
    pub fn config_dir() -> CalDirResult<PathBuf> {
        let config_dir_path = Self::platform_config_dir()?.join("caldir");

        #[cfg(target_os = "macos")]
        Self::migrate_legacy_macos_path(&config_dir_path)?;

        Ok(config_dir_path)
    }

    // ~/config/caldir/config.toml
    pub fn config_path() -> CalDirResult<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Serialize the config to TOML. Fields with a concrete default (like
    /// `calendar_dir` and `time_format`) are always emitted so the file
    /// documents the effective values; `Option` fields are omitted when
    /// `None` since there's no value to write.
    pub fn to_toml_string(&self) -> CalDirResult<String> {
        toml::to_string_pretty(self).map_err(|e| CalDirError::Config(e.to_string()))
    }

    /// Save the current config to ~/.config/caldir/config.toml
    pub fn save(&self) -> CalDirResult<()> {
        let config_path = Self::config_path()?;
        let content = self.to_toml_string()?;

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

    /// Make the macOS path ~/.config/caldir (like Linux)
    /// instead of the dirs::config_dir default of ~/Library/Application Support
    #[cfg(target_os = "macos")]
    fn platform_config_dir() -> CalDirResult<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine home directory".into()))?;
        Ok(home.join(".config"))
    }

    #[cfg(not(target_os = "macos"))]
    fn platform_config_dir() -> CalDirResult<PathBuf> {
        dirs::config_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine config directory".into()))
    }

    /// Migrates data from ~/Library/Application Support/caldir (legacy)
    /// to ~/.config/caldir (new)
    #[cfg(target_os = "macos")]
    fn migrate_legacy_macos_path(new_path: &std::path::Path) -> CalDirResult<()> {
        let home = dirs::home_dir()
            .ok_or_else(|| CalDirError::Config("Could not determine home directory".into()))?;

        let old_path = home
            .join("Library")
            .join("Application Support")
            .join("caldir");

        if !old_path.exists() {
            return Ok(());
        }

        if new_path.exists() {
            std::fs::remove_dir_all(&old_path)?;
            return Ok(());
        }

        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        eprintln!(
            "Migrating caldir config: {} → {}",
            old_path.display(),
            new_path.display()
        );

        std::fs::rename(&old_path, new_path)?;

        Ok(())
    }
}
