#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("event is missing a start time (DTSTART)")]
    MissingStart,
}
