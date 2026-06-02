use crate::provider::ProviderError;
use crate::rpc::ListCalendars;
use crate::{CalendarConfig, Provider};

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

    pub async fn list_calendars(&self) -> Result<Vec<CalendarConfig>, ProviderError> {
        self.provider
            .call(ListCalendars {
                account_identifier: self.identifier.clone(),
            })
            .await
    }
}
