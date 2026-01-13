//! Types for the Google Calendar provider.

use chrono::{DateTime, Utc};
use google_calendar::types::CalendarListEntry;
use serde::{Deserialize, Serialize};

// Re-export shared event types from caldir-core
pub use caldir_core::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency,
};

// =============================================================================
// Provider-specific types (not shared with caldir-core)
// =============================================================================

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}

impl GoogleCalendar {
    pub fn from_calendar_list_entry(entry: CalendarListEntry) -> Self {
        GoogleCalendar {
            id: entry.id,
            name: if entry.summary.is_empty() {
                "(unnamed)".to_string()
            } else {
                entry.summary
            },
            primary: entry.primary,
        }
    }
}
