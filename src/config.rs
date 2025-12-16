use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Directory to sync calendar events to
    #[serde(default = "default_calendar_dir")]
    pub calendar_dir: String,

    /// Provider configurations (OAuth credentials)
    #[serde(default)]
    pub providers: Providers,
}

#[derive(Debug, Default, Deserialize)]
pub struct Providers {
    pub gcal: Option<GcalConfig>,
}

/// OAuth credentials for Google Calendar
#[derive(Debug, Deserialize)]
pub struct GcalConfig {
    pub client_id: String,
    pub client_secret: String,
}

fn default_calendar_dir() -> String {
    "~/calendar".to_string()
}

/// Tokens storage: provider -> account email -> tokens
/// Example: { "gcal": { "user@gmail.com": { ... }, "work@company.com": { ... } } }
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(default)]
    pub gcal: HashMap<String, AccountTokens>,
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
            [providers.gcal]\n\
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
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}
