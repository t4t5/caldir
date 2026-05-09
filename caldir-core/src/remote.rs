mod config;

use crate::provider::ProviderError;
use crate::{Event, Provider};
pub use config::{RemoteConfig, RemoteConfigParams};

/// remote = a resolved provider with a config
pub struct Remote {
    provider: Provider,
    config: RemoteConfig,
}

impl Remote {
    pub fn new(provider: Provider, config: RemoteConfig) -> Self {
        Self { provider, config }
    }

    pub async fn create_event(&self, event: &Event) -> Result<Event, ProviderError> {
        self.provider
            .create_event(event.clone(), self.config.clone())
            .await
    }
}
