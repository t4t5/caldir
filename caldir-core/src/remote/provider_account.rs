use crate::calendar::config::CalendarConfig;
use crate::error::CalDirResult;
use crate::remote::protocol::{ListCalendars, ProviderRequestContext};
use crate::remote::provider::Provider;

pub struct ProviderAccount {
    pub provider: Provider,
    pub identifier: String,
    context: ProviderRequestContext,
}

impl ProviderAccount {
    pub fn new(provider: Provider, identifier: String, context: ProviderRequestContext) -> Self {
        ProviderAccount {
            provider,
            identifier,
            context,
        }
    }

    /// List all calendars for a provider account.
    ///
    /// Returns `CalendarConfig` for each calendar, which can be used to
    /// create local calendar directories with the correct remote configuration.
    pub async fn list_calendars(&self) -> CalDirResult<Vec<CalendarConfig>> {
        self.provider
            .call(
                &self.context,
                ListCalendars {
                    account_identifier: self.identifier.clone(),
                },
            )
            .await
    }
}
