use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};

// =============================================================================
// Wrapper Types for Type Safety
// =============================================================================

/// Provider enum - exhaustive list of supported providers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Google,
    Caldav,
    Ical,
}

/// Newtype for account email addresses
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountEmail(String);

impl AccountEmail {
    pub fn from_string(email: String) -> Self {
        Self(email)
    }
}

impl Deref for AccountEmail {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for AccountEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for calendar IDs (provider-specific identifiers)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CalendarId(String);

impl CalendarId {
    pub fn from_string(id: String) -> Self {
        Self(id)
    }
}

impl Deref for CalendarId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for CalendarId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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

    /// Calendar configurations (maps directory name to provider/account/calendar)
    #[serde(default)]
    pub calendars: HashMap<String, CalendarConfig>,

    /// Provider configurations (OAuth credentials)
    #[serde(default)]
    pub providers: Providers,
}

/// Configuration for a single calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub provider: Provider,
    pub account: AccountEmail,
    #[serde(default)]
    pub calendar_id: Option<CalendarId>, // None = primary
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Providers {
    pub google: Option<GoogleConfig>,
}

/// OAuth credentials for Google Calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
}

fn default_calendar_dir() -> String {
    "~/calendar".to_string()
}

/// Tokens storage: provider -> account email -> tokens
/// Example: { "google": { "user@gmail.com": { ... }, "work@company.com": { ... } } }
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(default)]
    pub google: HashMap<String, AccountTokens>,
}

/// Tokens for a single authenticated account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Get the config directory path (~/.config/caldir)
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

/// Get the tokens file path (~/.config/caldir/tokens.json)
pub fn tokens_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("tokens.json"))
}

/// Load config from ~/.config/caldir/config.toml
pub fn load_config() -> Result<Config> {
    let path = config_path()?;

    if !path.exists() {
        anyhow::bail!(
            "Config file not found at {}\n\n\
            Create it with your Google OAuth credentials:\n\n\
            [providers.google]\n\
            client_id = \"your-client-id.apps.googleusercontent.com\"\n\
            client_secret = \"your-client-secret\"\n\n\
            See CLAUDE.md for setup instructions.",
            path.display()
        );
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;

    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {}", path.display()))?;

    Ok(config)
}

/// Load tokens from ~/.config/caldir/tokens.json
pub fn load_tokens() -> Result<Tokens> {
    let path = tokens_path()?;

    if !path.exists() {
        return Ok(Tokens::default());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read tokens file at {}", path.display()))?;

    let tokens: Tokens = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse tokens file at {}", path.display()))?;

    Ok(tokens)
}

/// Save tokens to ~/.config/caldir/tokens.json
pub fn save_tokens(tokens: &Tokens) -> Result<()> {
    let path = tokens_path()?;

    // Ensure config directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory at {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(tokens)
        .context("Failed to serialize tokens")?;

    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write tokens file at {}", path.display()))?;

    Ok(())
}

/// Expand ~ in paths to the home directory
pub fn expand_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

/// Get the full path for a calendar directory
pub fn calendar_path(config: &Config, calendar_name: &str) -> PathBuf {
    expand_path(&config.calendar_dir).join(calendar_name)
}

/// Save config to ~/.config/caldir/config.toml
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;

    // Ensure config directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory at {}", parent.display()))?;
    }

    let contents = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;

    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write config file at {}", path.display()))?;

    Ok(())
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

/// Save sync state to calendar directory
pub fn save_sync_state(calendar_dir: &Path, state: &SyncState) -> Result<()> {
    let path = sync_state_path(calendar_dir);
    let contents = serde_json::to_string_pretty(state)
        .context("Failed to serialize sync state")?;
    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write sync state at {}", path.display()))?;
    Ok(())
}
