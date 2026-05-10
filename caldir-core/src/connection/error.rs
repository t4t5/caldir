use crate::calendar::CalendarEventError;
use crate::remote::RemoteError;

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Remote error: {0}")]
    Remote(#[from] RemoteError),

    #[error("Local calendar error: {0}")]
    CalendarEvent(#[from] CalendarEventError),
}
