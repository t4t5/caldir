use std::path::PathBuf;

use super::config::CalendarConfigError;
use super::state::CalendarStateError;
use crate::calendar::CalendarEventError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("calendar already exists at: {0}")]
    AlreadyExists(PathBuf),

    #[error("calendar not found at: {0}")]
    NotFound(PathBuf),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("calendar config error: {0}")]
    Config(#[from] CalendarConfigError),

    #[error("calendar state error: {0}")]
    State(#[from] CalendarStateError),

    #[error("calendar event error: {0}")]
    Event(#[from] CalendarEventError),

    #[error("master event not found: {0}")]
    MasterNotFound(String),

    #[error("event {0} is not recurring")]
    NotRecurring(String),
}
