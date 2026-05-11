use super::config::CaldirConfigError;
use crate::calendar::CalendarError;
use crate::provider::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum CaldirError {
    #[error(transparent)]
    Calendar(#[from] CalendarError),

    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("config error: {0}")]
    Config(#[from] CaldirConfigError),

    #[error("no default calendar configured")]
    NoDefaultCalendar,
}
