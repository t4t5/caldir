use std::path::PathBuf;

use super::config::CalendarConfigError;
use super::state::CalendarStateError;
use crate::calendar::CalendarEventError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarError {
    #[error("calendar already exists at: {0}")]
    AlreadyExists(PathBuf),

    #[error("calendar not found at: {0}")]
    NotFound(PathBuf),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Config(#[from] CalendarConfigError),

    #[error(transparent)]
    State(#[from] CalendarStateError),

    #[error(transparent)]
    Event(#[from] CalendarEventError),

    #[error("master event not found: {0}")]
    MasterNotFound(String),

    #[error("event {0} is not recurring")]
    NotRecurring(String),
}
