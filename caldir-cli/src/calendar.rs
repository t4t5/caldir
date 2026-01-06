use anyhow::Result;
use std::collections::HashSet;
use std::fmt;

use crate::caldir::Caldir;
use crate::config::CalendarConfig;
use crate::diff_new::CalendarDiff;
use crate::local_event::LocalEvent;
use crate::remote::Remote;
use crate::sync;

pub struct Calendar {
    pub name: String,
    pub config: CalendarConfig,
    pub caldir: Caldir,
}

impl Calendar {
    pub fn from(name: &str, caldir: &Caldir, config: &CalendarConfig) -> Self {
        Calendar {
            name: name.to_string(),
            caldir: caldir.clone(),
            config: config.clone(),
        }
    }

    /// Where the calendar's ics files are stored
    fn data_path(&self) -> std::path::PathBuf {
        self.caldir.data_path().join(&self.name)
    }

    /// Where changes get pushed to / pulled from
    pub fn remote(&self) -> Remote {
        Remote::from_calendar_config(&self.config)
    }

    /// Load events from local directory
    pub fn events(&self) -> Result<Vec<LocalEvent>> {
        let local_events = std::fs::read_dir(self.data_path())?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "ics"))
            .filter_map(|path| LocalEvent::from_file(path).ok())
            .collect();

        Ok(local_events)
    }

    /// UIDs we've seen before (for detecting deletions)
    pub fn seen_event_uids(&self) -> Result<HashSet<String>> {
        let state = sync::load_state(&self.data_path())?;
        Ok(state.synced_uids)
    }

    pub async fn get_diff(&self) -> Result<CalendarDiff> {
        CalendarDiff::from_calendar(self).await
    }

    pub fn render(&self) -> String {
        format!("🗓️ {}", self.name)
    }
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
