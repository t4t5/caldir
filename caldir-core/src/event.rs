//! Provider-neutral event types.
//!
//! These types represent calendar events in a provider-agnostic way.
//! Providers convert their API responses into these types, and caldir-cli
//! works exclusively with them for sync, diff, and ICS generation.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Typed recurrence data for a master event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recurrence {
    /// RRULE value (after "RRULE:"), e.g. "FREQ=WEEKLY;BYDAY=MO"
    pub rrule: String,
    /// Exception dates (EXDATE)
    pub exdates: Vec<EventTime>,
}

impl PartialEq for Recurrence {
    fn eq(&self, other: &Self) -> bool {
        if self.rrule != other.rrule {
            return false;
        }
        let mut a = self.exdates.clone();
        let mut b = other.exdates.clone();
        a.sort_by(|x, y| format!("{:?}", x).cmp(&format!("{:?}", y)));
        b.sort_by(|x, y| format!("{:?}", x).cmp(&format!("{:?}", y)));
        a == b
    }
}

/// A calendar event (provider-neutral)
///
/// `PartialEq` compares content fields only, ignoring sync metadata
/// (`updated`, `sequence`, `custom_properties`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// RFC 5545 UID - unique identifier shared across recurring event instances.
    /// Always present. For recurring events, the master and all instance overrides
    /// share the same uid, linked via recurrence_id.
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventTime,
    pub end: EventTime,
    pub status: EventStatus,

    // Recurrence fields
    /// Typed recurrence data (RRULE + EXDATEs) for master events
    pub recurrence: Option<Recurrence>,
    /// RECURRENCE-ID: Original start time for this instance.
    /// Present on instance overrides to identify which occurrence is modified.
    /// The shared uid links this override back to its master event.
    pub recurrence_id: Option<EventTime>,

    // Alarms & Availability
    pub reminders: Vec<Reminder>,
    /// Whether event blocks time (OPAQUE) or is free (TRANSPARENT)
    pub transparency: Transparency,

    // Meeting Data
    pub organizer: Option<Attendee>,
    pub attendees: Vec<Attendee>,
    pub conference_url: Option<String>,

    // Sync Infrastructure (excluded from PartialEq)
    /// Last modification timestamp (LAST-MODIFIED)
    pub updated: Option<DateTime<Utc>>,
    /// Revision sequence number (SEQUENCE)
    pub sequence: Option<i64>,

    // Provider-specific (excluded from PartialEq)
    /// Custom properties from the provider (e.g., X-GOOGLE-CONFERENCE)
    /// These are preserved for round-tripping back to the provider
    pub custom_properties: Vec<(String, String)>,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
            && self.summary == other.summary
            && self.description == other.description
            && self.location == other.location
            && self.start == other.start
            && self.end == other.end
            && self.status == other.status
            && self.recurrence == other.recurrence
            && self.recurrence_id == other.recurrence_id
            && self.reminders == other.reminders
            && self.transparency == other.transparency
            && self.organizer == other.organizer
            && self.attendees == other.attendees
            && self.conference_url == other.conference_url
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.summary.is_empty() {
            write!(f, "(Unknown Event)")
        } else {
            write!(f, "{}", self.summary)
        }
    }
}

impl Event {
    pub fn new(
        summary: String,
        start: EventTime,
        end: EventTime,
        description: Option<String>,
        location: Option<String>,
        recurrence: Option<Recurrence>,
        reminders: Vec<Reminder>,
    ) -> Self {
        let uid = format!("{}@caldir", uuid::Uuid::new_v4());

        Event {
            uid,
            summary,
            description,
            location,
            start,
            end,
            status: EventStatus::Confirmed,
            recurrence,
            recurrence_id: None,
            reminders,
            transparency: Transparency::Opaque,
            organizer: None,
            attendees: Vec::new(),
            conference_url: None,
            updated: None,
            sequence: None,
            custom_properties: Vec::new(),
        }
    }

    /// Render the event time with recurring indicator
    pub fn render_event_time(&self) -> String {
        let recurring = if self.recurrence.is_some() {
            " 🔁"
        } else {
            ""
        };
        format!("{}{}", self.start, recurring)
    }

    /// Returns the unique identifier for this event based on RFC 5545 identity.
    /// Format: `{uid}` for non-recurring events, `{uid}__{recurrence_id}` for instances.
    pub fn unique_id(&self) -> String {
        match &self.recurrence_id {
            Some(rid) => format!("{}__{}", self.uid, rid.to_ics_string()),
            None => self.uid.clone(),
        }
    }
}

/// An event attendee (also used for organizer)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attendee {
    pub name: Option<String>,
    pub email: String,
    /// Participation status (RFC 5545 PARTSTAT)
    pub response_status: Option<ParticipationStatus>,
}

/// Participation status for an attendee (RFC 5545 PARTSTAT)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum ParticipationStatus {
    Accepted,
    Declined,
    Tentative,
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

/// Event start/end time with timezone support
///
/// Supports three datetime forms (matching RFC 5545 / ICS format):
/// - UTC: explicit UTC time (DTSTART:20250320T150000Z)
/// - Floating: local time without timezone (DTSTART:20250320T150000)
/// - Zoned: time with explicit timezone (DTSTART;TZID=America/New_York:20250320T150000)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventTime {
    /// All-day event date (VALUE=DATE)
    Date(NaiveDate),
    /// UTC datetime (suffix Z)
    DateTimeUtc(DateTime<Utc>),
    /// Floating datetime - local time, no timezone
    /// Used for events that should happen at "9am wherever you are"
    DateTimeFloating(NaiveDateTime),
    /// Datetime with specific timezone (TZID parameter)
    DateTimeZoned {
        datetime: NaiveDateTime,
        tzid: String,
    },
}

impl EventTime {
    /// Get the start time as UTC DateTime (for comparison/sorting)
    /// Note: For floating and zoned times, this converts to UTC using naive interpretation
    pub fn to_utc(&self) -> Option<DateTime<Utc>> {
        match self {
            EventTime::Date(d) => d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc()),
            EventTime::DateTimeUtc(dt) => Some(*dt),
            EventTime::DateTimeFloating(dt) => Some(dt.and_utc()),
            EventTime::DateTimeZoned { datetime, .. } => Some(datetime.and_utc()),
        }
    }

    /// Check if this is an all-day date (not a datetime)
    pub fn is_date(&self) -> bool {
        matches!(self, EventTime::Date(_))
    }

    /// Format as ICS datetime string (for RECURRENCE-ID)
    pub fn to_ics_string(&self) -> String {
        match self {
            EventTime::Date(d) => d.format("%Y%m%d").to_string(),
            EventTime::DateTimeUtc(dt) => dt.format("%Y%m%dT%H%M%SZ").to_string(),
            EventTime::DateTimeFloating(dt) => dt.format("%Y%m%dT%H%M%S").to_string(),
            EventTime::DateTimeZoned { datetime, .. } => {
                datetime.format("%Y%m%dT%H%M%S").to_string()
            }
        }
    }

    /// Format as ISO 8601 string (for JSON/JavaScript compatibility)
    pub fn to_iso_string(&self) -> String {
        match self {
            EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
            EventTime::DateTimeUtc(dt) => dt.to_rfc3339(),
            EventTime::DateTimeFloating(dt) => format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")),
            EventTime::DateTimeZoned { datetime, .. } => {
                format!("{}Z", datetime.format("%Y-%m-%dT%H:%M:%S"))
            }
        }
    }
}

impl fmt::Display for EventTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventTime::Date(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            EventTime::DateTimeUtc(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M")),
            EventTime::DateTimeFloating(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M")),
            EventTime::DateTimeZoned { datetime, .. } => {
                write!(f, "{}", datetime.format("%Y-%m-%d %H:%M"))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}
