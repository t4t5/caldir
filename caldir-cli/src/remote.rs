use anyhow::Result;
use caldir_core::Event;
use std::collections::HashMap;

use crate::{config::CalendarConfig, provider::Provider};

use caldir_core::protocol::Command as ProviderCommand;

pub struct RemoteConfig(HashMap<String, toml::Value>);

impl RemoteConfig {
    fn from_calendar_config(config: &CalendarConfig) -> Self {
        RemoteConfig(config.params.clone())
    }

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
    config: RemoteConfig,
}

impl Remote {
    pub fn from_calendar_config(config: &CalendarConfig) -> Self {
        Remote {
            provider: Provider::from_name(&config.provider),
            config: RemoteConfig::from_calendar_config(config),
        }
    }

    pub async fn list_events(&self) -> Result<Vec<Event>> {
        self.provider
            .call(ProviderCommand::ListEvents, self.config.to_json())
            .await
    }
}
