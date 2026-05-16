use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventError {
    #[error("invalid event in ICS file {0}: {1}")]
    InvalidEvent(PathBuf, EventError),

    #[error("expected exactly one event in {path}, found {found}")]
    ExpectedSingleEvent { path: PathBuf, found: usize },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("event file not found: {0}")]
    NotFound(PathBuf),

    #[error("attendee not found: {email}")]
    AttendeeNotFound { email: String },
}
