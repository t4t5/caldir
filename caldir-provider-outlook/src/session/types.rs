//! Pure session data — no IO, no env vars.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// Whether this session was created via hosted (caldir.org) or local (self-hosted) OAuth.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    #[default]
    Local,
    Hosted,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Clone)]
pub struct Session {
    pub account_email: String,
    pub data: SessionData,
}

impl Session {
    pub fn new(account_email: &str, session_data: &SessionData, auth_mode: AuthMode) -> Self {
        let mut data = session_data.clone();
        data.auth_mode = auth_mode;
        Session {
            account_email: account_email.to_string(),
            data,
        }
    }

    pub fn access_token(&self) -> &str {
        &self.data.access_token
    }

    pub fn auth_mode(&self) -> &AuthMode {
        &self.data.auth_mode
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.data.expires_at
    }

    /// Filesystem slug. Forward-deterministic from the account email so
    /// `SessionStore::load` can compute the path directly. Preserved
    /// byte-for-byte from the pre-migration implementation — users may
    /// have existing files on disk under these names.
    pub(super) fn slug(account_email: &str) -> String {
        account_email.replace(['/', '\\', ':'], "_")
    }
}
