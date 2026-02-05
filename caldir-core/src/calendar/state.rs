//! Sync state tracking for calendars.

use std::collections::HashSet;

use crate::event::Event;
use crate::{calendar::Calendar, error::CalDirResult};

const KNOWN_EVENT_IDS_FILE: &str = "known_event_ids";

pub struct CalendarState {
    calendar: Calendar,
}

pub struct CalendarStateData {
    pub known_event_ids: Vec<String>,
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
    pub fn is_synced(&self, event: &Event) -> bool {
        self.known_event_ids().contains(&event.unique_id())
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
