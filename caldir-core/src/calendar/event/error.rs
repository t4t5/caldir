use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventError {
    #[error("invalid event in ICS file {0}: {1}")]
    InvalidEvent(PathBuf, EventError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
