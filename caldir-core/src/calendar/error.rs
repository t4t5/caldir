use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("invalid calendar path: {0}")]
    InvalidCalendarPath(PathBuf),

    #[error("calendar not found at: {0}")]
    NotFound(PathBuf),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
