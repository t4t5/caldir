//! Sync state tracking for calendars.

use std::collections::HashSet;

use crate::event::EventTime;
use crate::{calendar::Calendar, error::CalDirResult};

const KNOWN_EVENT_IDS_FILE: &str = "known_event_ids";

pub struct CalendarState {
    calendar: Calendar,
}

pub struct CalendarStateData {
    pub known_event_ids: Vec<String>,
}

/// Format an event identifier from (uid, recurrence_id).
/// Returns "uid" for non-recurring events, or "uid__recurrence_id" for recurring instances.
pub fn event_id(uid: &str, recurrence_id: Option<&EventTime>) -> String {
    match recurrence_id {
        Some(rid) => format!("{}__{}", uid, format_event_time(rid)),
        None => uid.to_string(),
    }
}

/// Format an EventTime as a string suitable for event ID (matches ICS format).
fn format_event_time(time: &EventTime) -> String {
    match time {
        EventTime::Date(d) => d.format("%Y%m%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y%m%dT%H%M%SZ").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y%m%dT%H%M%S").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y%m%dT%H%M%S").to_string(),
    }
}

impl CalendarState {
    pub fn load(calendar: Calendar) -> CalendarState {
        CalendarState { calendar }
    }

    fn path(&self) -> CalDirResult<std::path::PathBuf> {
        let dir = self.calendar.path()?;
        Ok(dir.join(".caldir/state"))
    }

    // Read .caldir/state/known_event_ids
    fn known_event_ids(&self) -> Vec<String> {
        let state_dir = match self.path() {
            Ok(dir) => dir,
            Err(_) => return vec![],
        };

        let path = state_dir.join(KNOWN_EVENT_IDS_FILE);

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => content
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect(),
                Err(_) => vec![],
            }
        } else {
            vec![]
        }
    }

    pub fn read(&self) -> CalendarStateData {
        let known_event_ids = self.known_event_ids();
        CalendarStateData { known_event_ids }
    }

    /// Check if an event has been synced (is in the known_event_ids set).
    pub fn is_synced(&self, uid: &str, recurrence_id: Option<&EventTime>) -> bool {
        let id = event_id(uid, recurrence_id);
        self.known_event_ids().contains(&id)
    }

    pub fn save(&self, event_ids: &HashSet<String>) -> CalDirResult<()> {
        let state_dir = self.path()?;
        std::fs::create_dir_all(&state_dir)?;

        let path = state_dir.join(KNOWN_EVENT_IDS_FILE);
        let temp = state_dir.join(KNOWN_EVENT_IDS_FILE.to_string() + ".tmp");

        // Sort for deterministic output
        let mut sorted: Vec<_> = event_ids.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        let content = sorted.join("\n");

        std::fs::write(&temp, content)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }
}
