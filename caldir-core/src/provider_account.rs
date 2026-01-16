use crate::protocol::Command as ProviderCommand;
use crate::{calendar_config::CalendarConfig, error::CalDirResult, provider::Provider};

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

    // List all calendars for a provider account
    pub async fn list_calendars(&self) -> CalDirResult<Vec<CalendarConfig>> {
        let params = serde_json::json!({ "account_identifier": self.identifier });

        self.provider
            .call_with_timeout(ProviderCommand::ListCalendars, params)
            .await
    }
}
