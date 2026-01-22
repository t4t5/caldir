//! Local event representation with file metadata.

use crate::calendar::Calendar;
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
    pub event: Event,
    pub path: PathBuf,                   // Path to the .ics file
    pub modified: Option<DateTime<Utc>>, // File modification time (used for sync direction detection)
}

impl CalendarEvent {
    pub fn new(path: PathBuf, event: &Event) -> Self {
        CalendarEvent {
            path,
            event: event.clone(),
            modified: None,
        }
    }

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

    // TODO: When saving, verify that base slug still matches. Otherwise rename file
    pub fn save(&self) -> CalDirResult<()> {
        let ics_content = crate::ics::generate_ics(&self.event)?;
        std::fs::write(&self.path, ics_content)?;
        Ok(())
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
    pub fn is_newer_than(&self, other_event: &Event) -> bool {
        let this_update_time = match self.modified {
            Some(mtime) => mtime,
            None => return false,
        };

        let other_update_time = match other_event.updated {
            Some(updated) => updated,
            None => return false,
        };

        this_update_time > other_update_time
    }

    pub fn unique_slug_for(event: &Event, calendar: &Calendar) -> CalDirResult<String> {
        let data_path = calendar.path()?;
        let base = Self::base_slug_for(event);

        // Try base slug first (check with .ics extension)
        if !data_path.join(format!("{}.ics", base)).exists() {
            return Ok(base);
        }

        // Collision - try suffixes
        for n in 2..=100 {
            let suffixed = format!("{}-{}", base, n);
            if !data_path.join(format!("{}.ics", suffixed)).exists() {
                return Ok(suffixed);
            }
        }

        Err(CalDirError::Config(format!(
            "Too many calendar name collisions for '{}'",
            base
        )))
    }

    /// Generate the base slug for this event.
    /// Timed events: `YYYY-MM-DDTHHMM__slug`
    /// All-day events: `YYYY-MM-DD__slug`
    /// Recurring events: `_recurring__slug`
    fn base_slug_for(event: &Event) -> String {
        let slug = slugify(&event.summary);

        if event.recurrence.is_some() {
            return format!("_recurring__{}", slug);
        }

        let date = match &event.start {
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
