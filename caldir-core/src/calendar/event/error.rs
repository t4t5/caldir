use std::path::PathBuf;

use crate::event::EventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventError {
    #[error("failed to parse ics file {0}: {1}")]
    IcsParse(PathBuf, String),

    #[error("no event found in ics file: {0}")]
    NoEventInIcs(PathBuf),

    #[error("invalid event in ics file {0}: {1}")]
    InvalidEvent(PathBuf, EventError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
