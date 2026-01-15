use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use google_calendar::{AccessToken, Client};
use serde::{Deserialize, Serialize};

use crate::app_config::{AppConfig, base_dir};

pub struct Session {
    account_email: String,
    data: SessionData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionData {
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
}

impl From<&AccessToken> for SessionData {
    fn from(tokens: &AccessToken) -> Self {
        let expires_at = Utc::now() + Duration::seconds(tokens.expires_in);

        SessionData {
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            expires_at,
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

    pub fn new(account_email: &str, session_data: &SessionData) -> Result<Self> {
        let session = Session {
            account_email: account_email.to_string(),
            data: session_data.clone(),
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
        let app_config = AppConfig::load()?;

        let client = Client::new(
            app_config.client_id,
            app_config.client_secret,
            String::new(),
            self.data.access_token.clone(),
            self.data.refresh_token.clone(),
        );

        let tokens = client
            .refresh_access_token()
            .await
            .context("Failed to refresh token")?;

        let session_data: SessionData = (&tokens).into();

        self.data = session_data;
        self.save()?;

        Ok(())
    }
}
