//! CalDAV-specific remote configuration.

use anyhow::Result;
use caldir_core::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly-typed remote configuration for generic CalDAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaldavRemoteConfig {
    pub caldav_account: String,
    pub caldav_calendar_url: String,
}

impl CaldavRemoteConfig {
    pub fn new(account: impl Into<String>, calendar_url: impl Into<String>) -> Self {
        Self {
            caldav_account: account.into(),
            caldav_calendar_url: calendar_url.into(),
        }
    }
}

impl From<CaldavRemoteConfig> for RemoteConfig {
    fn from(config: CaldavRemoteConfig) -> Self {
        let mut map = HashMap::new();
        map.insert(
            "caldav_account".to_string(),
            toml::Value::String(config.caldav_account),
        );
        map.insert(
            "caldav_calendar_url".to_string(),
            toml::Value::String(config.caldav_calendar_url),
        );
        RemoteConfig(map)
    }
}

impl TryFrom<&serde_json::Map<String, serde_json::Value>> for CaldavRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(map: &serde_json::Map<String, serde_json::Value>) -> Result<Self> {
        let caldav_account = map
            .get("caldav_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: caldav_account"))?
            .to_string();

        let caldav_calendar_url = map
            .get("caldav_calendar_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: caldav_calendar_url"))?
            .to_string();

        Ok(Self {
            caldav_account,
            caldav_calendar_url,
        })
    }
}
