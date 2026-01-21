//! Local event representation with file metadata.

use crate::constants::DEFAULT_SYNC_DAYS;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventTime};
use crate::ics::parse_event;
use crate::utils::slugify;
use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;

/// A local calendar event (stored as an ics file)
#[derive(Debug, Clone)]
pub struct CalendarEvent {
    /// Path to the .ics file
    pub path: PathBuf,
    /// The event data
    pub event: Event,
    /// File modification time (used for sync direction detection)
    pub modified: Option<DateTime<Utc>>,
}

impl CalendarEvent {
    pub fn from_file(path: PathBuf) -> CalDirResult<Self> {
        let content = std::fs::read_to_string(&path)?;

        let event = parse_event(&content).ok_or_else(|| {
            CalDirError::IcsParse(format!("Failed to parse event from {}", path.display()))
        })?;

        // Get file modification time for sync direction detection
        let modified = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from);

        Ok(CalendarEvent {
            path,
            event,
            modified,
        })
    }

    /// Check if this event falls within the sync window (±SYNC_DAYS from now).
    pub fn is_in_sync_range(&self) -> bool {
        let now = Utc::now();
        let range_start = now - Duration::days(DEFAULT_SYNC_DAYS);
        let range_end = now + Duration::days(DEFAULT_SYNC_DAYS);

        match self.event.start.to_utc() {
            Some(start) => start >= range_start && start <= range_end,
            None => true,
        }
    }

    /// Check if local file was modified after the remote event was updated.
    pub fn is_newer_than(&self, remote: &Event) -> bool {
        match (self.modified, remote.updated) {
            (Some(local_mtime), Some(remote_updated)) => local_mtime > remote_updated,
            _ => false,
        }
    }

    /// Generate the base slug for this event.
    /// Timed events: `YYYY-MM-DDTHHMM__slug`
    /// All-day events: `YYYY-MM-DD__slug`
    /// Recurring events: `_recurring__slug`
    pub fn base_slug(&self) -> String {
        let slug = slugify(&self.event.summary);

        if self.event.recurrence.is_some() {
            return format!("_recurring__{}", slug);
        }

        let date = match &self.event.start {
            EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
            EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
            EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
            EventTime::DateTimeZoned { datetime, .. } => {
                datetime.format("%Y-%m-%dT%H%M").to_string()
            }
        };

        format!("{}__{}", date, slug)
    }
}
