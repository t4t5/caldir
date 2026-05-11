mod config;
mod error;
mod event;

use crate::provider::ProviderError;
use crate::{Event, Provider, rpc};

pub use config::{RemoteConfig, RemoteConfigParams};
pub(crate) use error::RemoteError;
pub use event::RemoteEvent;

/// provider with config should resolve to a unique remote
pub struct Remote {
    provider: Provider,
    params: RemoteConfigParams,
}

impl Remote {
    pub fn new(provider: Provider, params: RemoteConfigParams) -> Self {
        Self { provider, params }
    }

    pub async fn list_events(&self) -> Result<Vec<RemoteEvent>, RemoteError> {
        let events = self
            .provider
            .call(rpc::ListEvents {
                remote: self.params.clone(),
            })
            .await?
            .into_iter()
            .map(RemoteEvent::new)
            .collect();

        Ok(events)
    }

    pub async fn create_event(&self, event: Event) -> Result<RemoteEvent, RemoteError> {
        let event = self
            .provider
            .call(rpc::CreateEvent {
                remote: self.params.clone(),
                event,
            })
            .await?;

        Ok(RemoteEvent::new(event))
    }

    pub async fn delete_event(&self, event: Event) -> Result<(), RemoteError> {
        self.provider
            .call(rpc::DeleteEvent {
                remote: self.params.clone(),
                event,
            })
            .await?;

        Ok(())
    }
}
