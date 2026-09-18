use crate::calendar::CalendarError;
use crate::remote::RemoteError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectionError {
    #[error(transparent)]
    Remote(#[from] RemoteError),

    #[error(transparent)]
    Calendar(#[from] CalendarError),
}
