use super::config::CaldirConfigError;
use crate::calendar::CalendarError;
use crate::provider::ProviderError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaldirError {
    #[error(transparent)]
    Calendar(#[from] CalendarError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Config(#[from] CaldirConfigError),

    #[error("no default calendar configured")]
    NoDefaultCalendar,
}
