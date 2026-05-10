mod config;

use crate::provider::ProviderError;
use crate::{Event, Provider, rpc};
pub use config::{RemoteConfig, RemoteConfigParams};

/// provider with config should resolve to a unique remote
pub struct Remote {
    provider: Provider,
    params: RemoteConfigParams,
}

impl Remote {
    pub fn new(provider: Provider, params: RemoteConfigParams) -> Self {
        Self { provider, params }
    }

    pub async fn create_event(&self, event: Event) -> Result<Event, ProviderError> {
        self.provider
            .call(rpc::CreateEvent {
                remote: self.params.clone(),
                event,
            })
            .await
    }

    pub async fn list_events(&self) -> Result<Vec<Event>, ProviderError> {
        self.provider
            .call(rpc::ListEvents {
                remote: self.params.clone(),
            })
            .await
    }
}
