//! Shared types for the provider protocol.
//!
//! These types mirror the ones in caldir-cli but are defined locally
//! to keep the provider self-contained.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Config Types
// =============================================================================

/// OAuth credentials for Google Calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
}

/// Tokens for a single authenticated account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

// =============================================================================
// Calendar Types
// =============================================================================

/// A calendar from the provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}

// =============================================================================
// Event Types
// =============================================================================

/// A calendar event (provider-neutral)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventTime,
    pub end: EventTime,
    pub status: EventStatus,

    /// RRULE, EXDATE lines for master events
    pub recurrence: Option<Vec<String>>,
    /// Original start time for this instance (used for RECURRENCE-ID)
    pub original_start: Option<EventTime>,

    /// Reminders/alarms for this event
    pub reminders: Vec<Reminder>,
    /// Whether event blocks time (OPAQUE) or is free (TRANSPARENT)
    pub transparency: Transparency,

    /// Event organizer
    pub organizer: Option<Attendee>,
    /// Event attendees/participants
    pub attendees: Vec<Attendee>,
    /// Conference/video call URL
    pub conference_url: Option<String>,

    /// Last modification timestamp (LAST-MODIFIED)
    pub updated: Option<DateTime<Utc>>,
    /// Revision sequence number (SEQUENCE)
    pub sequence: Option<i64>,

    /// Custom properties from the provider (e.g., X-GOOGLE-CONFERENCE)
    pub custom_properties: Vec<(String, String)>,
}

/// An event attendee (also used for organizer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    /// Display name
    pub name: Option<String>,
    /// Email address
    pub email: String,
    /// Response status: "accepted", "declined", "tentative", "needsAction"
    pub response_status: Option<String>,
}

/// A reminder/alarm for an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    /// Minutes before the event to trigger
    pub minutes: i64,
}

/// Event transparency (busy/free status)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transparency {
    /// Event blocks time on calendar (default)
    Opaque,
    /// Event does not block time (shows as free)
    Transparent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventTime {
    DateTime(DateTime<Utc>),
    Date(NaiveDate),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}
