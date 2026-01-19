//! Remote calendar operations via providers.

use std::collections::HashMap;

use crate::constants::DEFAULT_SYNC_DAYS;
use crate::error::{CalDirError, CalDirResult};
use crate::event::Event;
use crate::remote::protocol::Command as ProviderCommand;
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
    fn json_params(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::from(&self.config)
    }

    pub async fn events(&self) -> CalDirResult<Vec<Event>> {
        let now = chrono::Utc::now();
        let from = (now - Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();
        let to = (now + Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();

        let mut params = self.json_params();
        params.insert("from".into(), from.into());
        params.insert("to".into(), to.into());

        self.provider
            .call_with_timeout(
                ProviderCommand::ListEvents,
                serde_json::Value::Object(params),
            )
            .await
    }

    pub async fn create_event(&self, event: &Event) -> CalDirResult<Event> {
        let mut params = self.json_params();
        params.insert(
            "event".into(),
            serde_json::to_value(event).map_err(|e| CalDirError::Serialization(e.to_string()))?,
        );

        self.provider
            .call_with_timeout(
                ProviderCommand::CreateEvent,
                serde_json::Value::Object(params),
            )
            .await
    }

    pub async fn update_event(&self, event: &Event) -> CalDirResult<Event> {
        let mut params = self.json_params();
        params.insert(
            "event".into(),
            serde_json::to_value(event).map_err(|e| CalDirError::Serialization(e.to_string()))?,
        );

        self.provider
            .call_with_timeout(
                ProviderCommand::UpdateEvent,
                serde_json::Value::Object(params),
            )
            .await
    }

    pub async fn delete_event(&self, event_id: &str) -> CalDirResult<()> {
        let mut params = self.json_params();
        params.insert("event_id".into(), event_id.into());

        self.provider
            .call_with_timeout(
                ProviderCommand::DeleteEvent,
                serde_json::Value::Object(params),
            )
            .await
    }
}
