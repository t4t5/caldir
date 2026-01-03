use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// =============================================================================
// Configuration Structures
// =============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Directory to sync calendar events to
    #[serde(default = "default_calendar_dir")]
    pub calendar_dir: String,

    /// Default calendar for new events
    #[serde(default)]
    pub default_calendar: Option<String>,

    /// Calendar configurations (maps directory name to provider config)
    #[serde(default)]
    pub calendars: HashMap<String, CalendarConfig>,
}

/// Configuration for a single calendar.
///
/// The `provider` field specifies which provider binary to use (e.g., "google").
/// All other fields are provider-specific and passed to the provider as-is.
///
/// Example:
/// ```toml
/// [calendars.personal]
/// provider = "google"
/// google_account = "me@gmail.com"
/// google_calendar_id = "primary"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    /// Provider name (e.g., "google", "caldav", "ical")
    /// Maps to binary: caldir-provider-{name}
    pub provider: String,

    /// Provider-specific parameters (google_account, google_calendar_id, etc.)
    #[serde(flatten)]
    pub params: HashMap<String, toml::Value>,
}

fn default_calendar_dir() -> String {
    "~/calendar".to_string()
}

/// Get the config directory path (~/.config/caldir or platform equivalent)
pub fn config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir");
    Ok(config_dir)
}

/// Get the config file path (~/.config/caldir/config.toml)
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Load config from ~/.config/caldir/config.toml
pub fn load_config() -> Result<Config> {
    let path = config_path()?;

    if !path.exists() {
        anyhow::bail!(
            "Config file not found at {}\n\n\
            Create it with your calendar configuration:\n\n\
            calendar_dir = \"~/calendar\"\n\n\
            [calendars.personal]\n\
            provider = \"google\"\n\
            google_account = \"your-email@gmail.com\"\n\
            google_calendar_id = \"primary\"\n\n\
            Then run: caldir auth google",
            path.display()
        );
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;

    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {}", path.display()))?;

    Ok(config)
}

/// Expand ~ in paths to the home directory
pub fn expand_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }
    PathBuf::from(path)
}

/// Get the full path for a calendar directory
pub fn calendar_path(config: &Config, calendar_name: &str) -> PathBuf {
    expand_path(&config.calendar_dir).join(calendar_name)
}

// =============================================================================
// Sync State (for tracking which events have been synced)
// =============================================================================

/// Tracks which event UIDs have been synced for a calendar.
/// Used to detect local deletions (UID in state but no local file).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SyncState {
    pub synced_uids: HashSet<String>,
}

/// Get sync state file path for a calendar directory
pub fn sync_state_path(calendar_dir: &Path) -> PathBuf {
    calendar_dir.join(".caldir-sync")
}

/// Load sync state from calendar directory
pub fn load_sync_state(calendar_dir: &Path) -> Result<SyncState> {
    let path = sync_state_path(calendar_dir);
    if !path.exists() {
        return Ok(SyncState::default());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read sync state at {}", path.display()))?;
    let state: SyncState = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse sync state at {}", path.display()))?;
    Ok(state)
}

/// Save sync state to calendar directory (atomic write via temp file + rename)
pub fn save_sync_state(calendar_dir: &Path, state: &SyncState) -> Result<()> {
    let path = sync_state_path(calendar_dir);
    let temp_path = calendar_dir.join(".caldir-sync.tmp");

    let contents =
        serde_json::to_string_pretty(state).context("Failed to serialize sync state")?;

    // Write to temp file first
    std::fs::write(&temp_path, contents)
        .with_context(|| format!("Failed to write temp sync state at {}", temp_path.display()))?;

    // Atomic rename (on POSIX systems, rename is atomic if same filesystem)
    std::fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename sync state to {}", path.display()))?;

    Ok(())
}
