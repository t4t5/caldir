//! App-level configuration for the Google provider.
//!
//! User-provided OAuth credentials stored at:
//!   ~/.config/caldir/providers/google/app_config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Google OAuth client credentials (user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

pub fn base_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir")
        .join("providers")
        .join("google"))
}

pub fn load() -> Result<Credentials> {
    let path = base_dir()?.join("app_config.toml");

    if !path.exists() {
        anyhow::bail!(
            "Google credentials not found.\n\n\
            Create {} with:\n\n\
            client_id = \"your-client-id.apps.googleusercontent.com\"\n\
            client_secret = \"your-client-secret\"\n\n\
            See https://console.cloud.google.com/apis/credentials for setup.",
            path.display()
        );
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read credentials from {}", path.display()))?;

    let creds: Credentials = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse credentials from {}", path.display()))?;

    Ok(creds)
}
