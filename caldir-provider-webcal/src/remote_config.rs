//! Webcal-specific remote configuration.

use anyhow::Result;
use caldir_core::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly-typed remote configuration for webcal subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcalRemoteConfig {
    pub webcal_account: String,
    pub webcal_url: String,
}

impl WebcalRemoteConfig {
    pub fn new(account: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            webcal_account: account.into(),
            webcal_url: url.into(),
        }
    }
}

impl From<WebcalRemoteConfig> for RemoteConfig {
    fn from(config: WebcalRemoteConfig) -> Self {
        let mut map = HashMap::new();
        map.insert(
            "webcal_account".to_string(),
            toml::Value::String(config.webcal_account),
        );
        map.insert(
            "webcal_url".to_string(),
            toml::Value::String(config.webcal_url),
        );
        RemoteConfig(map)
    }
}

impl TryFrom<&serde_json::Map<String, serde_json::Value>> for WebcalRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(map: &serde_json::Map<String, serde_json::Value>) -> Result<Self> {
        let webcal_account = map
            .get("webcal_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: webcal_account"))?
            .to_string();

        let webcal_url = map
            .get("webcal_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: webcal_url"))?
            .to_string();

        Ok(Self {
            webcal_account,
            webcal_url,
        })
    }
}
