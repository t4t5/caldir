use crate::calendar::Calendar;
use crate::error::CalDirResult;
use crate::remote::protocol::Command as ProviderCommand;
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

    // List all calendars for a provider account
    pub async fn list_calendars(&self) -> CalDirResult<Vec<Calendar>> {
        let params = serde_json::json!({ "account_identifier": self.identifier });

        self.provider
            .call_with_timeout(ProviderCommand::ListCalendars, params)
            .await
    }
}
