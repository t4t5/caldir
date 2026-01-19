use crate::config::calendar_config::CalendarConfig;
use crate::error::CalDirResult;
use crate::remote::protocol::ListCalendars;
use crate::remote::provider::Provider;

pub struct ProviderAccount {
    pub provider: Provider,
    pub identifier: String,
}

impl ProviderAccount {
    pub fn new(provider: Provider, identifier: String) -> Self {
        ProviderAccount {
            provider,
            identifier,
        }
    }

    /// List all calendars for a provider account.
    ///
    /// Returns `CalendarConfig` for each calendar, which can be used to
    /// create local calendar directories with the correct remote configuration.
    pub async fn list_calendars(&self) -> CalDirResult<Vec<CalendarConfig>> {
        self.provider
            .call(ListCalendars {
                account_identifier: self.identifier.clone(),
            })
            .await
    }
}
