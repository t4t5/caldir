//! Types for the Google Calendar provider.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Google app OAuth credentials
/// Stored in ~/.config/caldir/providers/google/credentials.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
}

/// Google tokens for authenticated account.
/// Stored in ~/.config/caldir/providers/google/tokens/{account}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAccountTokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}
