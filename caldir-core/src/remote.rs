mod config;

use crate::provider::ProviderError;
use crate::{Event, Provider, rpc};
pub use config::{RemoteConfig, RemoteConfigParams};

/// provider with config should resolve to a unique remote
pub struct Remote {
    provider: Provider,
    config: RemoteConfig,
}

impl Remote {
    pub fn new(provider: Provider, config: RemoteConfig) -> Self {
        Self { provider, config }
    }

    pub async fn create_event(
        &self,
        event: Event,
        remote_config: RemoteConfig,
    ) -> Result<Event, ProviderError> {
        self.provider
            .call(rpc::CreateEvent {
                remote_config,
                event,
            })
            .await
    }
}
