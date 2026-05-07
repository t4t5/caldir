use std::path::PathBuf;

use super::config::CalendarConfigError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("invalid calendar path: {0}")]
    InvalidCalendarPath(PathBuf),

    #[error("calendar not found at: {0}")]
    NotFound(PathBuf),

    #[error("config error: {0}")]
    Config(#[from] CalendarConfigError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
