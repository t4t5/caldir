#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarStateError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    InvalidEvent(#[from] crate::event::EventError),
}
