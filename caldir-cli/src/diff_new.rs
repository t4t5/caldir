use caldir_core::Event;
use std::fmt;

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

impl CalendarDiff {
    pub fn is_empty(&self) -> bool {
        self.to_push.is_empty() && self.to_pull.is_empty()
    }
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
