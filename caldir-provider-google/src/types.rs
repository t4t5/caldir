//! Types for the Google Calendar provider.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export shared event types from caldir-core
pub use caldir_core::{Attendee, Event, EventStatus, EventTime, Reminder, Transparency};

// =============================================================================
// Provider-specific types (not shared with caldir-core)
// =============================================================================

/// OAuth credentials for Google Calendar.
/// Stored in ~/.config/caldir/providers/google/credentials.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
}

/// Tokens for a single authenticated account.
/// Stored in ~/.config/caldir/providers/google/tokens/{account}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// A calendar from the provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}
