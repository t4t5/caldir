//! Sync state tracking for calendars.

use std::collections::HashSet;

use crate::{calendar::Calendar, error::CalDirResult};

const KNOWN_UIDS_FILE: &str = "known_uids";

pub struct CalendarState {
    calendar: Calendar,
}

pub struct CalendarStateData {
    pub known_uids: Vec<String>,
}

impl CalendarState {
    pub fn load(calendar: Calendar) -> CalendarState {
        CalendarState { calendar }
    }

    fn state_dir(&self) -> CalDirResult<std::path::PathBuf> {
        let dir = self.calendar.dir()?;
        Ok(dir.join(".caldir/state"))
    }

    // Read .caldir/state/known_uids
    fn known_uids(&self) -> Vec<String> {
        let state_dir = match self.state_dir() {
            Ok(dir) => dir,
            Err(_) => return vec![],
        };

        let path = state_dir.join(KNOWN_UIDS_FILE);

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
        let known_uids = self.known_uids();
        CalendarStateData { known_uids }
    }

    pub fn save(&self, uids: &HashSet<String>) -> CalDirResult<()> {
        let dir = self.state_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = dir.join(KNOWN_UIDS_FILE);
        let temp = dir.join(KNOWN_UIDS_FILE.to_string() + ".tmp");

        // Sort for deterministic output
        let mut sorted: Vec<_> = uids.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        let content = sorted.join("\n");

        std::fs::write(&temp, content)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }
}
