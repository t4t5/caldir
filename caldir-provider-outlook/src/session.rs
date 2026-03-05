//! Creates a valid Outlook session (access token) for calling the Microsoft Graph API.

use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::app_config::{AppConfig, base_dir};

const TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const HOSTED_REFRESH_URL: &str = "https://caldir.org/auth/outlook/refresh";

/// Whether this session was created via hosted (caldir.org) or local (self-hosted) OAuth.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    #[default]
    Local,
    Hosted,
}

pub struct Session {
    account_email: String,
    data: SessionData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionData {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub auth_mode: AuthMode,
}

impl SessionData {
    pub fn from_tokens(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        let expires_at = Utc::now() + TimeDelta::seconds(expires_in);
        SessionData {
            access_token,
            refresh_token,
            expires_at,
            auth_mode: AuthMode::Local,
        }
    }

    pub fn from_hosted_tokens(
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    ) -> Self {
        let expires_at = Utc::now() + TimeDelta::seconds(expires_in);
        SessionData {
            access_token,
            refresh_token,
            expires_at,
            auth_mode: AuthMode::Hosted,
        }
    }
}

impl Session {
    fn path_for_account_email(account_email: &str) -> Result<std::path::PathBuf> {
        let email_slug = account_email.replace(['/', '\\', ':'], "_");
        Ok(base_dir()?
            .join("session")
            .join(format!("{}.toml", email_slug)))
    }

    fn path(&self) -> Result<std::path::PathBuf> {
        Self::path_for_account_email(&self.account_email)
    }

    pub fn access_token(&self) -> &str {
        &self.data.access_token
    }

    pub fn new(account_email: &str, session_data: SessionData, auth_mode: AuthMode) -> Self {
        let mut data = session_data;
        data.auth_mode = auth_mode;
        Session {
            account_email: account_email.to_string(),
            data,
        }
    }

    /// Load a session and refresh it if expired.
    pub async fn load_valid(account_email: &str) -> Result<Self> {
        let mut session = Self::load(account_email)?;
        if session.is_expired() {
            session.refresh().await?;
        }
        Ok(session)
    }

    fn load(account_email: &str) -> Result<Self> {
        let path = Self::path_for_account_email(account_email)?;
        if !path.exists() {
            anyhow::bail!("Outlook OAuth session for {} not found!", account_email);
        }
        let contents = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "Failed to read Outlook OAuth session from {}",
                path.display()
            )
        })?;
        let session_data: SessionData = toml::from_str(&contents).with_context(|| {
            format!(
                "Failed to parse Outlook OAuth session from {}",
                path.display()
            )
        })?;
        Ok(Session {
            account_email: account_email.to_string(),
            data: session_data,
        })
    }

    pub fn save(&self) -> Result<()> {
        let contents = toml::to_string_pretty(&self.data).context("Failed to serialize session")?;
        let path = self.path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        std::fs::write(&path, &contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;
        // Set to owner-only (0600) since file contains OAuth tokens:
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }
        Ok(())
    }

    fn is_expired(&self) -> bool {
        Utc::now() >= self.data.expires_at
    }

    async fn refresh(&mut self) -> Result<()> {
        match self.data.auth_mode {
            AuthMode::Hosted => self.refresh_hosted().await,
            AuthMode::Local => self.refresh_local().await,
        }
    }

    async fn refresh_hosted(&mut self) -> Result<()> {
        let client = reqwest::Client::new();

        let response = client
            .post(HOSTED_REFRESH_URL)
            .json(&serde_json::json!({
                "refresh_token": self.data.refresh_token,
            }))
            .send()
            .await
            .context("Failed to send refresh request to caldir.org")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to refresh Outlook token via caldir.org: {}",
                error_text
            );
        }

        #[derive(Deserialize)]
        struct RefreshResponse {
            access_token: String,
            refresh_token: String,
            expires_in: i64,
        }

        let refresh_data: RefreshResponse = response
            .json()
            .await
            .context("Failed to parse refresh response from caldir.org")?;

        self.data = SessionData::from_hosted_tokens(
            refresh_data.access_token,
            refresh_data.refresh_token,
            refresh_data.expires_in,
        );
        self.save()?;

        Ok(())
    }

    async fn refresh_local(&mut self) -> Result<()> {
        let app_config = AppConfig::load()?;

        let client = reqwest::Client::new();
        let response = client
            .post(TOKEN_ENDPOINT)
            .form(&[
                ("client_id", app_config.client_id.as_str()),
                ("client_secret", app_config.client_secret.as_str()),
                ("refresh_token", self.data.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("Failed to send refresh request to Microsoft")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to refresh Outlook token: {}", error_text);
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: String,
            expires_in: i64,
        }

        let tokens: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token refresh response")?;

        self.data =
            SessionData::from_tokens(tokens.access_token, tokens.refresh_token, tokens.expires_in);
        self.save()?;

        Ok(())
    }
}
