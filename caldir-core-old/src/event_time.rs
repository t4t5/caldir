use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

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
    /// Resolve to the UTC instant this event actually starts at, given a host
    /// timezone for interpreting wall-clock-only variants.
    ///
    /// - `Date`: midnight in `host_tz`.
    /// - `DateTimeFloating`: wall-clock interpreted in `host_tz` (RFC 5545
    ///   floating time = "9am wherever you are").
    /// - `DateTimeUtc` / `DateTimeZoned`: instant is already unambiguous;
    ///   `host_tz` is ignored.
    ///
    /// Use this (not `to_utc`) whenever the result drives a real-world time
    /// decision: range filtering, reminder scheduling, etc. `to_utc` is an
    /// ordering projection only.
    pub fn resolve_instant_in_zone<Tz: TimeZone>(&self, host_tz: &Tz) -> Option<DateTime<Utc>> {
        match self {
            EventTime::Date(d) => host_tz
                .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
                .single()
                .map(|l| l.with_timezone(&Utc)),
            EventTime::DateTimeFloating(dt) => host_tz
                .from_local_datetime(dt)
                .single()
                .map(|l| l.with_timezone(&Utc)),
            EventTime::DateTimeUtc(_) | EventTime::DateTimeZoned { .. } => self.to_utc(),
        }
    }

    /// Get the start time as UTC DateTime (for comparison/sorting)
    /// Note: For floating and zoned times, this converts to UTC using naive interpretation
    pub fn to_utc(&self) -> Option<DateTime<Utc>> {
        match self {
            EventTime::Date(d) => d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc()),
            EventTime::DateTimeUtc(dt) => Some(*dt),
            EventTime::DateTimeFloating(dt) => Some(dt.and_utc()),
            EventTime::DateTimeZoned { datetime, tzid } => {
                if let Ok(tz) = tzid.parse::<chrono_tz::Tz>()
                    && let Some(zoned) = datetime.and_local_timezone(tz).single()
                {
                    return Some(zoned.with_timezone(&Utc));
                }
                Some(datetime.and_utc())
            }
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

    /// Parse an ICS-format string back into an `EventTime`, using `template`
    /// to disambiguate floating vs. zoned (which share `YYYYMMDDTHHMMSS`).
    /// Used for round-tripping a synthetic instance ID (`unique_id()`) back
    /// into its `recurrence_id` component, where the master's `start` provides
    /// the template variant.
    pub fn from_ics_string_like(s: &str, template: &EventTime) -> Result<Self, String> {
        if s.len() == 8 && !s.contains('T') {
            return NaiveDate::parse_from_str(s, "%Y%m%d")
                .map(EventTime::Date)
                .map_err(|e| format!("invalid ICS date '{}': {}", s, e));
        }
        if s.ends_with('Z') {
            return NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ")
                .map(|dt| EventTime::DateTimeUtc(dt.and_utc()))
                .map_err(|e| format!("invalid ICS UTC datetime '{}': {}", s, e));
        }
        let dt = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
            .map_err(|e| format!("invalid ICS datetime '{}': {}", s, e))?;
        match template {
            EventTime::DateTimeZoned { tzid, .. } => Ok(EventTime::DateTimeZoned {
                datetime: dt,
                tzid: tzid.clone(),
            }),
            _ => Ok(EventTime::DateTimeFloating(dt)),
        }
    }

    /// Format as ISO 8601 string (for JSON/JavaScript compatibility)
    pub fn to_iso_string(&self) -> String {
        match self {
            EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
            _ => self.to_utc().map(|dt| dt.to_rfc3339()).unwrap_or_default(),
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
