use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

static DEFAULT_CALDIR_PATH: &str = "~/calendar";

fn default_caldir_path() -> PathBuf {
    PathBuf::from(DEFAULT_CALDIR_PATH)
}

// Parse the user settings in ~/.config/caldir/config.toml
#[derive(Deserialize, Clone)]
pub struct CaldirConfig {
    #[serde(default = "default_caldir_path")]
    pub calendar_dir: PathBuf,

    pub default_calendar: Option<String>,

    pub calendars: HashMap<String, CalendarConfig>,
}

impl CaldirConfig {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("caldir");

        Ok(config_dir.join("config.toml"))
    }
}

/// Configuration for a single calendar
///
/// Example:
/// [calendars.personal]
/// provider = "google"
/// google_account = "me@gmail.com"
/// google_calendar_id = "primary"
#[derive(Deserialize, Clone)]
pub struct CalendarConfig {
    /// "google", "caldav", "ical"... etc
    /// (specifies which provider binary to use)
    pub provider: String,

    /// Provider-specific params (google_account, google_calendar_id, etc.)
    /// (passed to the provider as-is)
    #[serde(flatten)]
    pub params: HashMap<String, toml::Value>,
}
