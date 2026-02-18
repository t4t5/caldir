pub mod protocol;
pub mod provider;
pub mod provider_account;

use std::collections::HashMap;

use crate::date_range::DateRange;
use crate::error::CalDirResult;
use crate::event::Event;
use crate::remote::protocol::{CreateEvent, DeleteEvent, ListEvents, UpdateEvent};
use crate::remote::provider::Provider;
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

    /// Returns the account identifier for this remote, if present.
    ///
    /// Looks for a `{provider}_account` field in the config (e.g., `google_account`,
    /// `icloud_account`). Providers that have an account concept should include this
    /// field in their remote config. Providers without accounts (e.g., plain CalDAV
    /// servers) simply omit it.
    pub fn account_identifier(&self) -> Option<&str> {
        let key = format!("{}_account", self.provider.name());
        self.config.0.get(&key).and_then(|v| v.as_str())
    }

    pub async fn events(&self, range: &DateRange) -> CalDirResult<Vec<Event>> {
        self.provider
            .call(ListEvents {
                remote_config: self.remote_config(),
                from: range.from_rfc3339(),
                to: range.to_rfc3339(),
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
