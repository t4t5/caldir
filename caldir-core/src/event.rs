//! Provider-neutral event types.
//!
//! These types represent calendar events in a provider-agnostic way.
//! Providers convert their API responses into these types, and caldir-cli
//! works exclusively with them for sync, diff, and ICS generation.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

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

    // Recurrence fields
    /// RRULE, EXDATE lines for master events
    pub recurrence: Option<Vec<String>>,
    /// Original start time for this instance (used for RECURRENCE-ID)
    pub original_start: Option<EventTime>,

    // Alarms & Availability
    /// Reminders/alarms for this event
    pub reminders: Vec<Reminder>,
    /// Whether event blocks time (OPAQUE) or is free (TRANSPARENT)
    pub transparency: Transparency,

    // Meeting Data
    /// Event organizer
    pub organizer: Option<Attendee>,
    /// Event attendees/participants
    pub attendees: Vec<Attendee>,
    /// Conference/video call URL
    pub conference_url: Option<String>,

    // Sync Infrastructure
    /// Last modification timestamp (LAST-MODIFIED)
    pub updated: Option<DateTime<Utc>>,
    /// Revision sequence number (SEQUENCE)
    pub sequence: Option<i64>,

    // Provider-specific
    /// Custom properties from the provider (e.g., X-GOOGLE-CONFERENCE)
    /// These are preserved for round-tripping back to the provider
    pub custom_properties: Vec<(String, String)>,
}

/// An event attendee (also used for organizer)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attendee {
    /// Display name
    pub name: Option<String>,
    /// Email address
    pub email: String,
    /// Participation status (RFC 5545 PARTSTAT)
    pub response_status: Option<ParticipationStatus>,
}

/// Participation status for an attendee (RFC 5545 PARTSTAT)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum ParticipationStatus {
    /// Attendee has accepted
    Accepted,
    /// Attendee has declined
    Declined,
    /// Attendee has tentatively accepted
    Tentative,
    /// Attendee needs to respond
    NeedsAction,
}

impl ParticipationStatus {
    /// Convert to ICS PARTSTAT string (RFC 5545)
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED",
            Self::Declined => "DECLINED",
            Self::Tentative => "TENTATIVE",
            Self::NeedsAction => "NEEDS-ACTION",
        }
    }

    /// Parse from ICS PARTSTAT string
    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "ACCEPTED" => Some(Self::Accepted),
            "DECLINED" => Some(Self::Declined),
            "TENTATIVE" => Some(Self::Tentative),
            "NEEDS-ACTION" => Some(Self::NeedsAction),
            _ => None,
        }
    }
}

/// A reminder/alarm for an event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
