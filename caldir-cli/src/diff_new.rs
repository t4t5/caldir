use anyhow::Result;
use caldir_core::Event;
use owo_colors::OwoColorize;
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
            DiffKind::Create => write!(f, "+"),
            DiffKind::Update => write!(f, "~"),
            DiffKind::Delete => write!(f, "-"),
        }
    }
}

impl DiffKind {
    /// Colorize text according to this diff kind
    pub fn colorize(&self, text: &str) -> String {
        match self {
            DiffKind::Create => text.green().to_string(),
            DiffKind::Update => text.to_string(),
            DiffKind::Delete => text.red().to_string(),
        }
    }

    /// Render the symbol with appropriate color
    pub fn render(&self) -> String {
        self.colorize(&self.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct EventDiff {
    pub kind: DiffKind,
    pub old: Option<Event>,
    pub new: Option<Event>,
}

impl fmt::Display for EventDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.event())
    }
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

    /// Get the event (prefer new, fallback to old)
    fn event(&self) -> &Event {
        self.new
            .as_ref()
            .or(self.old.as_ref())
            .expect("EventDiff must have at least one event")
    }

    pub fn render(&self) -> String {
        let event = self.event();
        let summary = self.kind.colorize(&event.to_string());
        let time = event.render_event_time();

        format!("{} {} {}", self.kind.render(), summary, time.dimmed())
    }
}

pub struct CalendarDiff {
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
}

impl CalendarDiff {
    pub fn is_empty(&self) -> bool {
        self.to_push.is_empty() && self.to_pull.is_empty()
    }

    pub fn render(&self) -> String {
        if self.is_empty() {
            return "   No changes".dimmed().to_string();
        }

        let mut lines = Vec::new();

        if !self.to_push.is_empty() {
            lines.push("   Local changes (to push):".dimmed().to_string());
            for diff in &self.to_push {
                lines.push(format!("   {}", diff.render()));
            }
        }

        if !self.to_pull.is_empty() {
            lines.push("   Remote changes (to pull):".dimmed().to_string());
            for diff in &self.to_pull {
                lines.push(format!("   {}", diff.render()));
            }
        }

        lines.join("\n")
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
        let sort_by_start = |a: &EventDiff, b: &EventDiff| {
            a.event().start.to_utc().cmp(&b.event().start.to_utc())
        };
        to_push.sort_by(sort_by_start);
        to_pull.sort_by(sort_by_start);

        Ok(CalendarDiff { to_push, to_pull })
    }
}
