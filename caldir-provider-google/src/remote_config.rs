//! Google-specific remote configuration.
//!
//! This provides type safety for Google Calendar remote config while
//! caldir-core remains provider-agnostic with its generic RemoteConfigParams.

use anyhow::Result;
use caldir_core::RemoteConfigParams;
use serde::{Deserialize, Serialize};

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

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "google_account".to_string(),
            toml::Value::String(self.google_account),
        );
        params.insert(
            "google_calendar_id".to_string(),
            toml::Value::String(self.google_calendar_id),
        );
        params
    }
}

impl TryFrom<&RemoteConfigParams> for GoogleRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let google_account = params
            .get("google_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: google_account"))?
            .to_string();

        let google_calendar_id = params
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
