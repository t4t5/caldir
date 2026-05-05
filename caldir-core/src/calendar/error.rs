use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("invalid calendar path: {0}")]
    InvalidCalendarPath(PathBuf),
}
