//! Outlook-specific remote configuration.

use anyhow::Result;
use caldir_core::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly-typed remote configuration for Outlook Calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlookRemoteConfig {
    pub outlook_account: String,
    pub outlook_calendar_id: String,
}

impl OutlookRemoteConfig {
    pub fn new(account: impl Into<String>, calendar_id: impl Into<String>) -> Self {
        Self {
            outlook_account: account.into(),
            outlook_calendar_id: calendar_id.into(),
        }
    }
}

impl From<OutlookRemoteConfig> for RemoteConfig {
    fn from(config: OutlookRemoteConfig) -> Self {
        let mut map = HashMap::new();
        map.insert(
            "outlook_account".to_string(),
            toml::Value::String(config.outlook_account),
        );
        map.insert(
            "outlook_calendar_id".to_string(),
            toml::Value::String(config.outlook_calendar_id),
        );
        RemoteConfig(map)
    }
}

impl TryFrom<&serde_json::Map<String, serde_json::Value>> for OutlookRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(map: &serde_json::Map<String, serde_json::Value>) -> Result<Self> {
        let outlook_account = map
            .get("outlook_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: outlook_account"))?
            .to_string();

        let outlook_calendar_id = map
            .get("outlook_calendar_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: outlook_calendar_id"))?
            .to_string();

        Ok(Self {
            outlook_account,
            outlook_calendar_id,
        })
    }
}
