#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventInstanceIdError {
    #[error("invalid recurrence id: {0}")]
    InvalidRecurrenceId(String),
}
