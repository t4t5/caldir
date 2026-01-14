//! OAuth session management for the Google provider.
//!
//! Handles token storage, refresh, and authenticated client creation.
//! Session data stored at: ~/.config/caldir/providers/google/session/{account}.toml

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_calendar::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::app_config::{self, Credentials};

/// An authenticated session for a Google account.
pub struct Session {
    pub email: String,
    creds: Credentials,
    tokens: Tokens,
}

impl Session {
    /// Load a session for the given account email.
    pub fn load(account_email: &str) -> Result<Self> {
        let creds = app_config::load()?;
        let tokens = Tokens::load(account_email)?;

        Ok(Self {
            email: account_email.to_string(),
            creds,
            tokens,
        })
    }

    /// Refresh tokens if they have expired, saving the new tokens to disk.
    pub async fn refresh_if_needed(&mut self) -> Result<()> {
        if !self.tokens.needs_refresh() {
            return Ok(());
        }

        let client = Client::new(
            self.creds.client_id.clone(),
            self.creds.client_secret.clone(),
            String::new(),
            self.tokens.access_token.clone(),
            self.tokens.refresh_token.clone(),
        );

        let access_token = client
            .refresh_access_token()
            .await
            .context("Failed to refresh token")?;

        let expires_at = if access_token.expires_in > 0 {
            Some(Utc::now() + chrono::Duration::seconds(access_token.expires_in))
        } else {
            None
        };

        // Google typically doesn't return a new refresh_token on refresh
        let refresh_token = if access_token.refresh_token.is_empty() {
            self.tokens.refresh_token.clone()
        } else {
            access_token.refresh_token
        };

        self.tokens = Tokens {
            access_token: access_token.access_token,
            refresh_token,
            expires_at,
        };

        self.tokens.save(&self.email)?;

        Ok(())
    }

    /// Get an authenticated Google Calendar API client.
    pub fn client(&self) -> Client {
        Client::new(
            self.creds.client_id.clone(),
            self.creds.client_secret.clone(),
            String::new(),
            self.tokens.access_token.clone(),
            self.tokens.refresh_token.clone(),
        )
    }
}

/// OAuth tokens for an authenticated account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl Tokens {
    fn path(account_email: &str) -> Result<PathBuf> {
        let safe_email = account_email.replace(['/', '\\', ':'], "_");
        Ok(app_config::base_dir()?
            .join("session")
            .join(format!("{}.toml", safe_email)))
    }

    pub fn load(account_email: &str) -> Result<Self> {
        let path = Self::path(account_email)?;

        if !path.exists() {
            anyhow::bail!(
                "No session for account: {}\n\
                Run `caldir-cli auth google` first.",
                account_email
            );
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session from {}", path.display()))?;

        let tokens: Tokens = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse session from {}", path.display()))?;

        Ok(tokens)
    }

    pub fn save(&self, account_email: &str) -> Result<()> {
        let path = Self::path(account_email)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create session directory at {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        // Restrict permissions to owner-only (0600) since this file contains OAuth tokens
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    pub fn needs_refresh(&self) -> bool {
        self.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false)
    }
}
