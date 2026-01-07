use anyhow::Result;
use caldir_core::Event;
use caldir_core::protocol::Command as ProviderCommand;
use std::collections::HashMap;

use crate::local::RemoteConfig;
use crate::provider::Provider;

/// Internal wrapper for provider params to convert to JSON
struct RemoteParams(HashMap<String, toml::Value>);

impl RemoteParams {
    fn to_json(&self) -> serde_json::Value {
        let json_map: serde_json::Map<String, serde_json::Value> = self
            .0
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    serde_json::to_value(v).unwrap_or(serde_json::Value::Null),
                )
            })
            .collect();
        serde_json::Value::Object(json_map)
    }
}

pub struct Remote {
    provider: Provider,
    params: RemoteParams,
}

impl Remote {
    pub fn from_remote_config(config: &RemoteConfig) -> Self {
        Remote {
            provider: Provider::from_name(&config.provider),
            params: RemoteParams(config.params.clone()),
        }
    }

    pub async fn events(&self) -> Result<Vec<Event>> {
        self.provider
            .call(ProviderCommand::ListEvents, self.params.to_json())
            .await
    }

    pub async fn create_event(&self, event: &Event) -> Result<Event> {
        let mut params = self.params.to_json();
        params["event"] = serde_json::to_value(event)?;
        self.provider
            .call(ProviderCommand::CreateEvent, params)
            .await
    }

    pub async fn update_event(&self, event: &Event) -> Result<Event> {
        let mut params = self.params.to_json();
        params["event"] = serde_json::to_value(event)?;
        self.provider
            .call(ProviderCommand::UpdateEvent, params)
            .await
    }

    pub async fn delete_event(&self, event_id: &str) -> Result<()> {
        let mut params = self.params.to_json();
        params["event_id"] = serde_json::Value::String(event_id.to_string());
        self.provider
            .call(ProviderCommand::DeleteEvent, params)
            .await
    }
}
