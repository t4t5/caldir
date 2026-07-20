use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("failed to read event from {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),

    #[error("failed to parse ICS {0}: {1}")]
    InvalidIcs(String, String),

    #[error("expected {expected} event(s) in ICS, found {found}")]
    UnexpectedEventCount { expected: usize, found: usize },

    #[error("event is missing a start time (DTSTART)")]
    MissingStart,

    #[error("event is missing a UID")]
    MissingUid,

    #[error("no attendee matching {email}")]
    AttendeeNotFound { email: String },
}
