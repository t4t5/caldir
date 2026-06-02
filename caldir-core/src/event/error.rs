#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("failed to parse ICS {0}: {1}")]
    InvalidIcs(String, String),

    #[error("event is missing a start time (DTSTART)")]
    MissingStart,

    #[error("event is missing a UID")]
    MissingUid,
}
