//! Calendar diff computation and application.

use std::collections::HashMap;

use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::sync::{DiffKind, EventDiff};

/// Represents the differences between local and remote calendar state.
pub struct CalendarDiff {
    pub calendar: Calendar,
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
}

impl CalendarDiff {
    pub fn is_empty(&self) -> bool {
        self.to_push.is_empty() && self.to_pull.is_empty()
    }

    pub async fn apply_push(&self) -> CalDirResult<()> {
        let remote = self
            .calendar
            .remote()
            .ok_or_else(|| CalDirError::NoRemoteConfigured(self.calendar.name.to_string()))?;

        for diff in &self.to_push {
            match diff.kind {
                DiffKind::Create => {
                    let event = diff.new.as_ref().expect("Create must have new event");
                    let created = remote.create_event(event).await?;
                    // Update local file with remote-assigned ID and fields
                    self.calendar.update_event(&event.id, &created)?;
                }
                DiffKind::Update => {
                    let event = diff.new.as_ref().expect("Update must have new event");
                    let updated = remote.update_event(event).await?;
                    // Update local file with any remote changes
                    self.calendar.update_event(&event.id, &updated)?;
                }
                DiffKind::Delete => {
                    let event = diff.old.as_ref().expect("Delete must have old event");
                    remote.delete_event(&event.id).await?;
                }
            }
        }

        self.calendar.update_sync_state()?;

        Ok(())
    }

    pub fn apply_pull(&self) -> CalDirResult<()> {
        for diff in &self.to_pull {
            match diff.kind {
                DiffKind::Create => {
                    let event = diff.new.as_ref().expect("Create must have new event");
                    self.calendar.create_event(event)?;
                }
                DiffKind::Update => {
                    let event = diff.new.as_ref().expect("Update must have new event");
                    self.calendar.update_event(&event.id, event)?;
                }
                DiffKind::Delete => {
                    let event = diff.old.as_ref().expect("Delete must have old event");
                    self.calendar.delete_event(&event.id)?;
                }
            }
        }

        self.calendar.update_sync_state()?;

        Ok(())
    }

    pub async fn from_calendar(calendar: &Calendar) -> CalDirResult<Self> {
        let remote = calendar
            .remote()
            .ok_or_else(|| CalDirError::NoRemoteConfigured(calendar.name.to_string()))?;

        let remote_events = remote.events().await?;
        let local_events = calendar.events()?;
        let seen_uids = calendar.seen_event_uids()?;

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

        let local_only_events = local_by_uid
            .iter()
            .filter(|(uid, _)| !remote_by_uid.contains_key(*uid));

        let remote_only_events = remote_by_uid
            .iter()
            .filter(|(uid, _)| !local_by_uid.contains_key(*uid));

        let shared_events = local_by_uid
            .iter()
            .filter_map(|(uid, local)| remote_by_uid.get(uid).map(|remote| (local, remote)));

        for (uid, local) in local_only_events {
            if seen_uids.contains(uid) {
                // Was synced before, now gone from remote → delete locally
                // But only if in sync range (old events weren't fetched, so we can't know)
                if local.is_in_sync_range() {
                    if let Some(diff) = EventDiff::get_diff(Some(local.event.clone()), None) {
                        to_pull.push(diff);
                    }
                }
            } else {
                // Never synced -> create on remote
                if let Some(diff) = EventDiff::get_diff(None, Some(local.event.clone())) {
                    to_push.push(diff);
                }
            }
        }

        for (uid, remote) in remote_only_events {
            if seen_uids.contains(uid) {
                // Was synced before, now gone locally -> delete on remote
                if let Some(diff) = EventDiff::get_diff(Some(remote.clone()), None) {
                    to_push.push(diff);
                }
            } else {
                // Never synced -> create locally
                if let Some(diff) = EventDiff::get_diff(None, Some(remote.clone())) {
                    to_pull.push(diff);
                }
            }
        }

        for (local, remote) in shared_events {
            if local.event == *remote {
                continue;
            }

            // Content differs - use timestamps to determine direction
            if local.is_newer_than(remote) {
                // Local was modified more recently -> push
                if let Some(diff) =
                    EventDiff::get_diff(Some(remote.clone()), Some(local.event.clone()))
                {
                    to_push.push(diff);
                }
            } else {
                // Remote was modified more recently -> pull
                if let Some(diff) =
                    EventDiff::get_diff(Some(local.event.clone()), Some(remote.clone()))
                {
                    to_pull.push(diff);
                }
            }
        }

        // Sort by event start time (ascending)
        let sort_by_start =
            |a: &EventDiff, b: &EventDiff| a.event().start.to_utc().cmp(&b.event().start.to_utc());
        to_push.sort_by(sort_by_start);
        to_pull.sort_by(sort_by_start);

        Ok(CalendarDiff {
            calendar: calendar.clone(),
            to_push,
            to_pull,
        })
    }
}
