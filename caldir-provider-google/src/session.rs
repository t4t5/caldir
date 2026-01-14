//! OAuth session management for the Google provider.
//!
//! Handles token storage, refresh, and authenticated client creation.
//! Tokens are stored at: ~/.config/caldir/providers/google/tokens/{account}.json

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_calendar::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::app_config::{self, Credentials};

pub const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

const REDIRECT_PORT: u16 = 8085;

pub fn redirect_uri() -> String {
    format!("http://localhost:{}/callback", REDIRECT_PORT)
}

pub fn redirect_address() -> String {
    format!("127.0.0.1:{}", REDIRECT_PORT)
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
    pub fn needs_refresh(&self) -> bool {
        self.expires_at
            .map(|exp| exp < Utc::now())
            .unwrap_or(false)
    }
}

/// An authenticated session for a Google account.
pub struct Session {
    pub email: String,
    creds: Credentials,
    tokens: Tokens,
}

impl Session {
    /// Load a session for the given account email.
    pub fn load(email: &str) -> Result<Self> {
        let creds = app_config::load()?;
        let tokens = load_tokens(email)?;

        Ok(Self {
            email: email.to_string(),
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
            redirect_uri(),
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

        save_tokens(&self.email, &self.tokens)?;

        Ok(())
    }

    /// Get an authenticated Google Calendar API client.
    pub fn client(&self) -> Client {
        Client::new(
            self.creds.client_id.clone(),
            self.creds.client_secret.clone(),
            redirect_uri(),
            self.tokens.access_token.clone(),
            self.tokens.refresh_token.clone(),
        )
    }
}

fn token_path(email: &str) -> Result<PathBuf> {
    let safe_email = email.replace(['/', '\\', ':'], "_");
    Ok(app_config::base_dir()?
        .join("tokens")
        .join(format!("{}.json", safe_email)))
}

fn load_tokens(email: &str) -> Result<Tokens> {
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

    let tokens: Tokens = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse tokens from {}", path.display()))?;

    Ok(tokens)
}

pub fn save_tokens(email: &str, tokens: &Tokens) -> Result<()> {
    let path = token_path(email)?;

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
