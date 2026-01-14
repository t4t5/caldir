use anyhow::Result;
use caldir_core::Event;
use caldir_core::protocol::Command as ProviderCommand;
use chrono::Duration;
use std::collections::HashMap;

use crate::constants::DEFAULT_SYNC_DAYS;
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

    fn ensure_account_param(&self) -> Result<()> {
        let account_key = format!("{}_account", self.provider.name());
        if !self.params.0.contains_key(&account_key) {
            anyhow::bail!(
                "Missing required remote config: {} (in .caldir/config.toml)",
                account_key
            );
        }
        Ok(())
    }

    pub async fn events(&self) -> Result<Vec<Event>> {
        let now = chrono::Utc::now();
        let from = (now - Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();
        let to = (now + Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();

        let mut params = self.params.to_json();
        params["from"] = serde_json::Value::String(from);
        params["to"] = serde_json::Value::String(to);

        self.provider
            .call(ProviderCommand::ListEvents, params)
            .await
    }

    pub async fn create_event(&self, event: &Event) -> Result<Event> {
        self.ensure_account_param()?;
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
        self.ensure_account_param()?;
        let mut params = self.params.to_json();
        params["event_id"] = serde_json::Value::String(event_id.to_string());
        self.provider
            .call(ProviderCommand::DeleteEvent, params)
            .await
    }
}
