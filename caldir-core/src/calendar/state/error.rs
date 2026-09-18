#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarStateError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid event: {0}")]
    InvalidEvent(#[from] crate::event::EventError),
}
