use crate::event::EventInstanceIdError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarStateError {
    #[error("calendar state error: {0}")]
    InvalidRecurrenceId(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("event instance error: {0}")]
    EventInstance(#[from] EventInstanceIdError),
}
