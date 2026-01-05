use crate::caldir::Caldir;
use crate::config::CalendarConfig;
use crate::diff_new::{Change, ChangeKind, Diff, Source};
use crate::local_event::LocalEvent;
use crate::remote::Remote;
use anyhow::Result;
use std::collections::HashMap;

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

    pub async fn get_diff(&self) -> Result<Diff> {
        let remote_events = self.remote().events().await?;
        let local_events = self.events()?;

        // Index local events by UID for fast lookup
        let local_by_uid: HashMap<String, LocalEvent> = local_events
            .into_iter()
            .map(|e| (e.event.id.clone(), e))
            .collect();

        let mut changes = Vec::new();

        for remote_event in remote_events {
            match local_by_uid.get(&remote_event.id) {
                Some(local_event) => {
                    if let Some(change) = local_event.diff_with(&remote_event) {
                        changes.push(change);
                    }
                }
                None => {
                    changes.push(Change {
                        source: Source::Remote,
                        kind: ChangeKind::Create,
                        local: None,
                        remote: Some(remote_event),
                    });
                }
            }
        }

        // TODO: Check for local-only events (push creates)
        // TODO: Handle deletes (requires sync state)

        Ok(Diff(changes))
    }
}
