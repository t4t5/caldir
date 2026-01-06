use anyhow::Result;
use caldir_core::Event;
use std::collections::HashMap;
use std::fmt;

use crate::calendar::Calendar;

#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    Create,
    Update,
    Delete,
}

impl fmt::Display for DiffKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffKind::Create => write!(f, "created"),
            DiffKind::Update => write!(f, "modified"),
            DiffKind::Delete => write!(f, "deleted"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventDiff {
    pub kind: DiffKind,
    pub old: Option<Event>,
    pub new: Option<Event>,
}

impl EventDiff {
    pub fn get_diff(old_event: Option<Event>, new_event: Option<Event>) -> Option<EventDiff> {
        match (&old_event, &new_event) {
            (None, Some(_)) => Some(EventDiff {
                kind: DiffKind::Create,
                old: None,
                new: new_event,
            }),
            (Some(_), None) => Some(EventDiff {
                kind: DiffKind::Delete,
                old: old_event,
                new: None,
            }),
            (Some(old), Some(new)) => {
                if old == new {
                    None
                } else {
                    Some(EventDiff {
                        kind: DiffKind::Update,
                        old: old_event,
                        new: new_event,
                    })
                }
            }
            (None, None) => None,
        }
    }

    /// Get the event summary (from new if available, otherwise old)
    pub fn summary(&self) -> &str {
        self.new
            .as_ref()
            .or(self.old.as_ref())
            .map(|e| e.summary.as_str())
            .unwrap_or("(unknown)")
    }
}

impl fmt::Display for EventDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.summary())
    }
}

pub struct CalendarDiff {
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
}

impl fmt::Display for CalendarDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return writeln!(f, "No changes");
        }

        if !self.to_push.is_empty() {
            writeln!(f, "To push:")?;
            for diff in &self.to_push {
                writeln!(f, "  {}", diff)?;
            }
        }

        if !self.to_pull.is_empty() {
            writeln!(f, "To pull:")?;
            for diff in &self.to_pull {
                writeln!(f, "  {}", diff)?;
            }
        }

        Ok(())
    }
}

impl CalendarDiff {
    pub fn is_empty(&self) -> bool {
        self.to_push.is_empty() && self.to_pull.is_empty()
    }

    pub async fn from_calendar(calendar: &Calendar) -> Result<Self> {
        let remote_events = calendar.remote().events().await?;
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
                if let Some(diff) = EventDiff::get_diff(Some(local.event.clone()), None) {
                    to_pull.push(diff);
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
            if let Some(diff) = EventDiff::get_diff(Some(local.event.clone()), Some(remote.clone()))
            {
                // Content differs -> pull remote version (remote is source of truth)
                to_pull.push(diff);
            }
        }

        Ok(CalendarDiff { to_push, to_pull })
    }
}
