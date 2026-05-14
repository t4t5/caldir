//! Filesystem-backed storage for [`Session`] credentials + OAuth refresh.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use chrono::{Duration, Utc};
use google_calendar::Client;
use serde::Deserialize;
use std::path::PathBuf;

use crate::app_config::AppConfigStore;

use super::types::{AuthMode, Session, SessionData};

const HOSTED_REFRESH_URL: &str = "https://caldir.org/auth/google/refresh";

/// Reads and writes [`Session`] files under a provider's storage root.
///
/// Layout: `{storage.root()}/session/{slug}.toml`, slug forward-deterministic
/// from the account email. Files contain OAuth tokens; on Unix they're
/// chmod'd to `0600`.
pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.path_for(&session.account_email);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create session directory: {}", parent.display())
            })?;
        }

        let contents =
            toml::to_string_pretty(&session.data).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        // Plaintext OAuth tokens — owner-only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Load a session by its account email.
    ///
    /// The slug is forward-deterministic from the email, so we compute the
    /// path directly rather than scanning the directory.
    pub fn load(&self, account_email: &str) -> Result<Session> {
        let path = self.path_for(account_email);

        if !path.exists() {
            anyhow::bail!("Google OAuth session for {} not found!", account_email);
        }

        let contents = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "Failed to read Google OAuth session from {}",
                path.display()
            )
        })?;

        let data: SessionData = toml::from_str(&contents).with_context(|| {
            format!(
                "Failed to parse Google OAuth session from {}",
                path.display()
            )
        })?;

        Ok(Session {
            account_email: account_email.to_string(),
            data,
        })
    }

    /// Load + auto-refresh if expired. `AuthMode::Local` refresh needs the
    /// user's OAuth client_id/secret from [`AppConfigStore`].
    pub async fn load_valid(
        &self,
        account_email: &str,
        app_config_store: &AppConfigStore,
    ) -> Result<Session> {
        let mut session = self.load(account_email)?;

        if session.is_expired() {
            self.refresh(&mut session, app_config_store).await?;
        }

        Ok(session)
    }

    /// Build a `google_calendar::Client` for an existing session.
    /// `AuthMode::Hosted` doesn't need app_config_store but takes it for symmetry.
    pub fn client(&self, session: &Session, app_config_store: &AppConfigStore) -> Result<Client> {
        match session.auth_mode() {
            AuthMode::Hosted => Ok(Client::new(
                String::new(),
                String::new(),
                String::new(),
                session.data.access_token.clone(),
                session.data.refresh_token.clone(),
            )),
            AuthMode::Local => {
                let app_config = app_config_store.load()?;
                Ok(Client::new(
                    app_config.client_id,
                    app_config.client_secret,
                    String::new(),
                    session.data.access_token.clone(),
                    session.data.refresh_token.clone(),
                ))
            }
        }
    }

    async fn refresh(
        &self,
        session: &mut Session,
        app_config_store: &AppConfigStore,
    ) -> Result<()> {
        match session.auth_mode() {
            AuthMode::Hosted => self.refresh_hosted(session).await,
            AuthMode::Local => self.refresh_local(session, app_config_store).await,
        }
    }

    async fn refresh_hosted(&self, session: &mut Session) -> Result<()> {
        let client = reqwest::Client::new();

        let response = client
            .post(HOSTED_REFRESH_URL)
            .json(&serde_json::json!({
                "refresh_token": session.data.refresh_token,
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

        session.data.access_token = refresh_data.access_token;
        session.data.expires_at = Utc::now() + Duration::seconds(refresh_data.expires_in);
        self.save(session)?;

        Ok(())
    }

    async fn refresh_local(
        &self,
        session: &mut Session,
        app_config_store: &AppConfigStore,
    ) -> Result<()> {
        let app_config = app_config_store.load()?;

        let client = Client::new(
            app_config.client_id,
            app_config.client_secret,
            String::new(),
            session.data.access_token.clone(),
            session.data.refresh_token.clone(),
        );

        let mut tokens = client
            .refresh_access_token()
            .await
            .context("Failed to refresh token")?;

        // Google typically doesn't return a new refresh_token on refresh
        if tokens.refresh_token.is_empty() {
            tokens.refresh_token = session.data.refresh_token.clone();
        }

        let mut new_data: SessionData = (&tokens).into();
        new_data.auth_mode = AuthMode::Local;
        session.data = new_data;
        self.save(session)?;

        Ok(())
    }

    fn session_dir(&self) -> PathBuf {
        self.storage.root().join("session")
    }

    fn path_for(&self, account_email: &str) -> PathBuf {
        self.session_dir()
            .join(format!("{}.toml", Session::slug(account_email)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn store() -> (TempDir, SessionStore) {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(ProviderStorage::new(tmp.path()));
        (tmp, store)
    }

    fn sample_session() -> Session {
        Session {
            account_email: "alice@gmail.com".to_string(),
            data: SessionData {
                access_token: "access-abc".to_string(),
                refresh_token: "refresh-xyz".to_string(),
                expires_at: Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap(),
                auth_mode: AuthMode::Hosted,
            },
        }
    }

    #[test]
    fn save_writes_toml_under_session_subdir() {
        let (tmp, store) = store();
        let session = sample_session();

        store.save(&session).unwrap();

        let expected = tmp
            .path()
            .join("session")
            .join(format!("{}.toml", Session::slug(&session.account_email)));
        assert!(
            expected.is_file(),
            "session file should exist at {expected:?}"
        );
    }

    #[test]
    fn load_round_trips_by_account_email() {
        let (_tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let loaded = store.load(&session.account_email).unwrap();

        assert_eq!(loaded.account_email, session.account_email);
        assert_eq!(loaded.data.access_token, session.data.access_token);
        assert_eq!(loaded.data.refresh_token, session.data.refresh_token);
        assert_eq!(loaded.data.auth_mode, session.data.auth_mode);
    }

    #[test]
    fn load_errors_when_missing() {
        let (_tmp, store) = store();
        let err = store.load("ghost@gmail.com").unwrap_err();
        assert!(err.to_string().contains("ghost@gmail.com"));
    }

    #[test]
    fn slug_preserves_pre_migration_email_layout() {
        // Google's slug replaces only / \ : — NOT @ or . — and existing
        // on-disk files use this exact form. Don't drift.
        assert_eq!(Session::slug("alice@gmail.com"), "alice@gmail.com");
        assert_eq!(Session::slug("a:b/c\\d"), "a_b_c_d");
    }

    #[cfg(unix)]
    #[test]
    fn save_chmods_session_file_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let path = tmp
            .path()
            .join("session")
            .join(format!("{}.toml", Session::slug(&session.account_email)));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
