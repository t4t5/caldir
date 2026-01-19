//! Remote calendar operations via providers.

use std::collections::HashMap;

use crate::constants::DEFAULT_SYNC_DAYS;
use crate::error::CalDirResult;
use crate::event::Event;
use crate::remote::protocol::{CreateEvent, DeleteEvent, ListEvents, UpdateEvent};
use crate::remote::provider::Provider;
use chrono::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteConfig(pub HashMap<String, toml::Value>);

impl From<&RemoteConfig> for serde_json::Map<String, serde_json::Value> {
    fn from(config: &RemoteConfig) -> Self {
        config
            .0
            .iter()
            .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|v| (k.clone(), v)))
            .collect()
    }
}

/// Remote provider configuration (e.g., Google Calendar settings)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Remote {
    pub provider: Provider,
    #[serde(flatten)]
    pub config: RemoteConfig,
}

impl Remote {
    fn remote_config(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::from(&self.config)
    }

    pub fn new(provider: Provider, config: RemoteConfig) -> Self {
        Remote { provider, config }
    }

    pub async fn events(&self) -> CalDirResult<Vec<Event>> {
        let now = chrono::Utc::now();
        let from = (now - Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();
        let to = (now + Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();

        self.provider
            .call(ListEvents {
                remote_config: self.remote_config(),
                from,
                to,
            })
            .await
    }

    pub async fn create_event(&self, event: &Event) -> CalDirResult<Event> {
        self.provider
            .call(CreateEvent {
                remote_config: self.remote_config(),
                event: event.clone(),
            })
            .await
    }

    pub async fn update_event(&self, event: &Event) -> CalDirResult<Event> {
        self.provider
            .call(UpdateEvent {
                remote_config: self.remote_config(),
                event: event.clone(),
            })
            .await
    }

    pub async fn delete_event(&self, event_id: &str) -> CalDirResult<()> {
        self.provider
            .call(DeleteEvent {
                remote_config: self.remote_config(),
                event_id: event_id.to_string(),
            })
            .await
    }
}
