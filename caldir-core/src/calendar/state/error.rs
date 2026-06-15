use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarStateError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid snapshot {0}: {1}")]
    InvalidSnapshot(PathBuf, EventError),

    #[error("snapshot {path} should contain exactly one event, found {found}")]
    InvalidSnapshotCount { path: PathBuf, found: usize },
}
