use crate::local_event::LocalEvent;
use caldir_core::Event;
use std::fmt;

/// Where the change originated
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Local,  // Change originated locally → needs push
    Remote, // Change originated remotely → needs pull
}

/// What kind of change
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeKind {
    Create,
    Update,
    Delete,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeKind::Create => write!(f, "new"),
            ChangeKind::Update => write!(f, "modified"),
            ChangeKind::Delete => write!(f, "deleted"),
        }
    }
}

/// A single change between local and remote state
#[derive(Debug)]
pub struct Change {
    pub source: Source,
    pub kind: ChangeKind,
    pub local: Option<LocalEvent>,
    pub remote: Option<Event>,
}

impl Change {
    pub fn summary(&self) -> &str {
        self.local
            .as_ref()
            .map(|l| l.event.summary.as_str())
            .or_else(|| self.remote.as_ref().map(|r| r.summary.as_str()))
            .unwrap_or("?")
    }
}

/// The diff between local and remote calendars
pub struct Diff(pub Vec<Change>);

impl Diff {
    /// Changes that need to be pushed (local → remote)
    pub fn to_push(&self) -> impl Iterator<Item = &Change> {
        self.0.iter().filter(|c| c.source == Source::Local)
    }

    /// Changes that need to be pulled (remote → local)
    pub fn to_pull(&self) -> impl Iterator<Item = &Change> {
        self.0.iter().filter(|c| c.source == Source::Remote)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return writeln!(f, "Up to date");
        }

        let to_pull: Vec<_> = self.to_pull().collect();
        let to_push: Vec<_> = self.to_push().collect();

        if !to_pull.is_empty() {
            writeln!(f, "To pull ({}):", to_pull.len())?;
            for change in to_pull {
                writeln!(f, "  {} {}", change.kind, change.summary())?;
            }
        }

        if !to_push.is_empty() {
            writeln!(f, "To push ({}):", to_push.len())?;
            for change in to_push {
                writeln!(f, "  {} {}", change.kind, change.summary())?;
            }
        }

        Ok(())
    }
}
