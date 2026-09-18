use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarEventError {
    #[error("invalid event in ICS file {0}")]
    InvalidEvent(PathBuf, #[source] EventError),

    #[error("expected exactly one event in {path}, found {found}")]
    ExpectedSingleEvent { path: PathBuf, found: usize },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("event file not found: {0}")]
    NotFound(PathBuf),

    #[error(transparent)]
    Event(#[from] EventError),

    #[error("event {0} is not a recurring master")]
    NotRecurring(String),
}
