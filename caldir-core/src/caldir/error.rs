use crate::calendar::CalendarError;

#[derive(Debug, thiserror::Error)]
pub enum CaldirError {
    #[error(transparent)]
    Calendar(#[from] CalendarError),
}
