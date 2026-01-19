//! Google-specific remote configuration.
//!
//! This provides type safety for Google Calendar remote config while
//! caldir-core remains provider-agnostic with its generic RemoteConfig.

use anyhow::Result;
use caldir_core::remote::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly-typed remote configuration for Google Calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleRemoteConfig {
    pub google_account: String,
    pub google_calendar_id: String,
}

impl GoogleRemoteConfig {
    pub fn new(account: impl Into<String>, calendar_id: impl Into<String>) -> Self {
        Self {
            google_account: account.into(),
            google_calendar_id: calendar_id.into(),
        }
    }
}

impl From<GoogleRemoteConfig> for RemoteConfig {
    fn from(config: GoogleRemoteConfig) -> Self {
        let mut map = HashMap::new();
        map.insert(
            "google_account".to_string(),
            toml::Value::String(config.google_account),
        );
        map.insert(
            "google_calendar_id".to_string(),
            toml::Value::String(config.google_calendar_id),
        );
        RemoteConfig(map)
    }
}

impl TryFrom<&serde_json::Map<String, serde_json::Value>> for GoogleRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(map: &serde_json::Map<String, serde_json::Value>) -> Result<Self> {
        let google_account = map
            .get("google_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: google_account"))?
            .to_string();

        let google_calendar_id = map
            .get("google_calendar_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: google_calendar_id"))?
            .to_string();

        Ok(Self {
            google_account,
            google_calendar_id,
        })
    }
}
