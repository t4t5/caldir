use std::fmt;

use serde::{Deserialize, Serialize};

use crate::event::Event;

use crate::diff::DiffKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn event(&self) -> &Event {
        self.new
            .as_ref()
            .or(self.old.as_ref())
            .expect("EventDiff must have at least one event")
    }
}
