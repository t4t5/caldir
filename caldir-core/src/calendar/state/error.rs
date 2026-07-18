use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarStateError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid event base {0}: {1}")]
    InvalidEventBase(PathBuf, EventError),

    #[error("event base {path} should contain exactly one event, found {found}")]
    InvalidEventBaseCount { path: PathBuf, found: usize },
}
