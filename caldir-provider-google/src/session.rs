//! Creates a valid Google session (access token) that we can use to call the gcal API

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use google_calendar::{AccessToken, Client};
use serde::{Deserialize, Serialize};

use crate::app_config::{AppConfig, base_dir};

const HOSTED_REFRESH_URL: &str = "https://caldir.org/auth/google/refresh";

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
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
    #[serde(default)]
    auth_mode: AuthMode,
}

impl From<&AccessToken> for SessionData {
    fn from(tokens: &AccessToken) -> Self {
        let expires_at = Utc::now() + Duration::seconds(tokens.expires_in);

        SessionData {
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            expires_at,
            auth_mode: AuthMode::Local,
        }
    }
}

impl SessionData {
    pub fn from_tokens(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        let expires_at = Utc::now() + Duration::seconds(expires_in);

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

    pub fn client(&self) -> Result<Client> {
        match self.data.auth_mode {
            AuthMode::Hosted => {
                // Hosted mode: no local client_id/secret needed for API calls
                Ok(Client::new(
                    String::new(),
                    String::new(),
                    String::new(),
                    self.data.access_token.clone(),
                    self.data.refresh_token.clone(),
                ))
            }
            AuthMode::Local => {
                let app_config = AppConfig::load()?;

                Ok(Client::new(
                    app_config.client_id,
                    app_config.client_secret,
                    String::new(),
                    self.data.access_token.clone(),
                    self.data.refresh_token.clone(),
                ))
            }
        }
    }

    pub fn new(account_email: &str, session_data: &SessionData, auth_mode: AuthMode) -> Result<Self> {
        let mut data = session_data.clone();
        data.auth_mode = auth_mode;

        let session = Session {
            account_email: account_email.to_string(),
            data,
        };
        Ok(session)
    }

    // Load a session and refresh it if expired:
    pub async fn load_valid(account_email: &str) -> Result<Self> {
        let session = Self::load(account_email)?;

        if session.is_expired() {
            let mut session = session;
            session.refresh().await?;
            Ok(session)
        } else {
            Ok(session)
        }
    }

    fn load(account_email: &str) -> Result<Self> {
        let path = Self::path_for_account_email(account_email)?;

        if !path.exists() {
            anyhow::bail!("Google OAuth session for {} not found!", account_email);
        }

        let contents = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "Failed to read Google OAuth session from {}",
                path.display()
            )
        })?;

        let session_data: SessionData = toml::from_str(&contents).with_context(|| {
            format!(
                "Failed to parse Google Oauth session from {}",
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

        std::fs::write(&path, contents)
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
            anyhow::bail!("Failed to refresh token via caldir.org: {}", error_text);
        }

        #[derive(Deserialize)]
        struct RefreshResponse {
            access_token: String,
            expires_in: i64,
        }

        let refresh_data: RefreshResponse = response
            .json()
            .await
            .context("Failed to parse refresh response from caldir.org")?;

        self.data.access_token = refresh_data.access_token;
        self.data.expires_at = Utc::now() + Duration::seconds(refresh_data.expires_in);
        self.save()?;

        Ok(())
    }

    async fn refresh_local(&mut self) -> Result<()> {
        let app_config = AppConfig::load()?;

        let client = Client::new(
            app_config.client_id,
            app_config.client_secret,
            String::new(),
            self.data.access_token.clone(),
            self.data.refresh_token.clone(),
        );

        let mut tokens = client
            .refresh_access_token()
            .await
            .context("Failed to refresh token")?;

        // Google typically doesn't return a new refresh_token on refresh
        if tokens.refresh_token.is_empty() {
            tokens.refresh_token = self.data.refresh_token.clone();
        }

        let session_data: SessionData = (&tokens).into();

        self.data = session_data;
        self.save()?;

        Ok(())
    }
}
