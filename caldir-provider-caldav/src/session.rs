//! Credential storage for generic CalDAV authentication.
//!
//! Stores server URL, username, password, and discovered URLs at:
//!   ~/.config/caldir/providers/caldav/session/{slug}.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::constants::PROVIDER_NAME;

pub fn base_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir")
        .join("providers")
        .join(PROVIDER_NAME))
}

/// Generic CalDAV session (credentials + discovered URLs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub server_url: String,
    pub username: String,
    pub password: String,
    /// User-specific CalDAV principal URL (discovered during auth)
    pub principal_url: String,
    /// Calendar home URL (discovered during auth)
    pub calendar_home_url: String,
}

impl Session {
    /// Derive a slug from username and server host for use as filename.
    fn slug(username: &str, server_url: &str) -> String {
        let host = url::Url::parse(server_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let raw = format!("{}@{}", username, host);
        raw.replace(['/', '\\', ':', '@', '.'], "_")
    }

    fn path_for(username: &str, server_url: &str) -> Result<PathBuf> {
        let slug = Self::slug(username, server_url);
        Ok(base_dir()?.join("session").join(format!("{}.toml", slug)))
    }

    fn path(&self) -> Result<PathBuf> {
        Self::path_for(&self.username, &self.server_url)
    }

    /// Build an account identifier like "user@host".
    pub fn account_identifier(username: &str, server_url: &str) -> String {
        let host = url::Url::parse(server_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        format!("{}@{}", username, host)
    }

    pub fn new(
        server_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        principal_url: impl Into<String>,
        calendar_home_url: impl Into<String>,
    ) -> Self {
        Session {
            server_url: server_url.into(),
            username: username.into(),
            password: password.into(),
            principal_url: principal_url.into(),
            calendar_home_url: calendar_home_url.into(),
        }
    }

    pub fn load(account_identifier: &str) -> Result<Self> {
        // account_identifier is "user@host" — we need to find the session file
        // by scanning the session directory since slug encoding may differ
        let session_dir = base_dir()?.join("session");
        if !session_dir.exists() {
            anyhow::bail!("CalDAV session for {} not found!", account_identifier);
        }

        for entry in std::fs::read_dir(&session_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                let contents = std::fs::read_to_string(&path)?;
                if let Ok(session) = toml::from_str::<Session>(&contents) {
                    let id = Self::account_identifier(&session.username, &session.server_url);
                    if id == account_identifier {
                        return Ok(session);
                    }
                }
            }
        }

        anyhow::bail!("CalDAV session for {} not found!", account_identifier);
    }

    pub fn save(&self) -> Result<()> {
        let path = self.path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create session directory: {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(&self).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        // Set to owner-only (0600) since file contains credentials
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Get credentials as (username, password) tuple for HTTP basic auth
    pub fn credentials(&self) -> (&str, &str) {
        (&self.username, &self.password)
    }
}
