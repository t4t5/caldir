//! Configuration and token storage for the Google provider.
//!
//! Credentials and tokens are stored in:
//!   ~/.config/caldir/providers/google/credentials.json
//!   ~/.config/caldir/providers/google/tokens/{account}.json

use crate::types::{GoogleAccountTokens, GoogleCredentials};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the provider's config directory (~/.config/caldir/providers/google)
pub fn provider_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir")
        .join("providers")
        .join("google");
    Ok(config_dir)
}

/// Get the credentials file path
pub fn credentials_path() -> Result<PathBuf> {
    Ok(provider_dir()?.join("credentials.json"))
}

/// Get the tokens directory path
pub fn tokens_dir() -> Result<PathBuf> {
    Ok(provider_dir()?.join("tokens"))
}

/// Get the token file path for a specific account
pub fn token_path(email: &str) -> Result<PathBuf> {
    // Sanitize account name for use as filename
    let safe_account = email.replace(['/', '\\', ':'], "_");
    Ok(tokens_dir()?.join(format!("{}.json", safe_account)))
}

/// Load credentials from disk
pub fn load_credentials() -> Result<GoogleCredentials> {
    let path = credentials_path()?;

    if !path.exists() {
        anyhow::bail!(
            "Google credentials not found.\n\n\
            Create {} with:\n\n\
            {{\n  \
              \"client_id\": \"your-client-id.apps.googleusercontent.com\",\n  \
              \"client_secret\": \"your-client-secret\"\n\
            }}\n\n\
            See https://console.cloud.google.com/apis/credentials for setup.",
            path.display()
        );
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read credentials from {}", path.display()))?;

    let creds: GoogleCredentials = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse credentials from {}", path.display()))?;

    Ok(creds)
}

/// Load tokens for a specific account
pub fn load_tokens(email: &str) -> Result<GoogleAccountTokens> {
    let path = token_path(email)?;

    if !path.exists() {
        anyhow::bail!(
            "No tokens for account: {}\n\
            Run `caldir-cli auth google` first.",
            email
        );
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read tokens from {}", path.display()))?;

    let tokens: GoogleAccountTokens = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse tokens from {}", path.display()))?;

    Ok(tokens)
}

/// Save tokens for a specific account
pub fn save_tokens(account: &str, tokens: &GoogleAccountTokens) -> Result<()> {
    let path = token_path(account)?;

    // Ensure tokens directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create tokens directory at {}", parent.display())
        })?;
    }

    let contents = serde_json::to_string_pretty(tokens).context("Failed to serialize tokens")?;

    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write tokens to {}", path.display()))?;

    Ok(())
}

/// Check if tokens are expired and need refresh
pub fn tokens_need_refresh(tokens: &GoogleAccountTokens) -> bool {
    tokens
        .expires_at
        .map(|exp| exp < chrono::Utc::now())
        .unwrap_or(false)
}
