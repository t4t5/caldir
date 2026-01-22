//! iCloud-specific remote configuration.
//!
//! This provides type safety for iCloud Calendar remote config while
//! caldir-core remains provider-agnostic with its generic RemoteConfig.

use anyhow::Result;
use caldir_core::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly-typed remote configuration for iCloud Calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ICloudRemoteConfig {
    pub icloud_account: String,
    pub icloud_calendar_url: String,
}

impl ICloudRemoteConfig {
    pub fn new(account: impl Into<String>, calendar_url: impl Into<String>) -> Self {
        Self {
            icloud_account: account.into(),
            icloud_calendar_url: calendar_url.into(),
        }
    }
}

impl From<ICloudRemoteConfig> for RemoteConfig {
    fn from(config: ICloudRemoteConfig) -> Self {
        let mut map = HashMap::new();
        map.insert(
            "icloud_account".to_string(),
            toml::Value::String(config.icloud_account),
        );
        map.insert(
            "icloud_calendar_url".to_string(),
            toml::Value::String(config.icloud_calendar_url),
        );
        RemoteConfig(map)
    }
}

impl TryFrom<&serde_json::Map<String, serde_json::Value>> for ICloudRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(map: &serde_json::Map<String, serde_json::Value>) -> Result<Self> {
        let icloud_account = map
            .get("icloud_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: icloud_account"))?
            .to_string();

        let icloud_calendar_url = map
            .get("icloud_calendar_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: icloud_calendar_url"))?
            .to_string();

        Ok(Self {
            icloud_account,
            icloud_calendar_url,
        })
    }
}
