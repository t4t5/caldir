use crate::caldir::Caldir;
use crate::config::CalendarConfig;
use crate::diff_new::{CalendarDiff, EventDiff};
use crate::local_event::LocalEvent;
use crate::remote::Remote;
use crate::sync;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

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
        let remote_events = self.remote().events().await?;
        let local_events = self.events()?;
        let seen_uids = self.seen_event_uids()?;

        let local_by_uid: HashMap<_, _> = local_events
            .into_iter()
            .map(|e| (e.event.id.clone(), e))
            .collect();

        let remote_by_uid: HashMap<_, _> = remote_events
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect();

        let mut to_push = Vec::new();
        let mut to_pull = Vec::new();

        // Local-only events
        for (uid, local) in &local_by_uid {
            if !remote_by_uid.contains_key(uid) {
                if seen_uids.contains(uid) {
                    // Was synced before, now gone from remote → delete locally
                    if let Some(diff) = EventDiff::get_diff(Some(local.event.clone()), None) {
                        to_pull.push(diff);
                    }
                } else {
                    // Never synced → create on remote
                    if let Some(diff) = EventDiff::get_diff(None, Some(local.event.clone())) {
                        to_push.push(diff);
                    }
                }
            }
        }

        // Remote-only events
        for (uid, remote) in &remote_by_uid {
            if !local_by_uid.contains_key(uid) {
                if seen_uids.contains(uid) {
                    // Was synced before, now gone locally → delete on remote
                    if let Some(diff) = EventDiff::get_diff(Some(remote.clone()), None) {
                        to_push.push(diff);
                    }
                } else {
                    // Never synced → create locally
                    if let Some(diff) = EventDiff::get_diff(None, Some(remote.clone())) {
                        to_pull.push(diff);
                    }
                }
            }
        }

        // Events in both → check if content differs
        for (uid, local) in &local_by_uid {
            if let Some(remote) = remote_by_uid.get(uid) {
                if let Some(diff) =
                    EventDiff::get_diff(Some(local.event.clone()), Some(remote.clone()))
                {
                    // Content differs - pull remote version (remote is source of truth)
                    to_pull.push(diff);
                }
            }
        }

        Ok(CalendarDiff { to_push, to_pull })
    }
}
