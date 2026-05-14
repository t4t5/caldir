//! Outlook-specific remote configuration.
//!
//! This provides type safety for Outlook Calendar remote config while
//! caldir-core remains provider-agnostic with its generic RemoteConfigParams.

use anyhow::Result;
use caldir_core::RemoteConfigParams;
use serde::{Deserialize, Serialize};

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

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "outlook_account".to_string(),
            toml::Value::String(self.outlook_account),
        );
        params.insert(
            "outlook_calendar_id".to_string(),
            toml::Value::String(self.outlook_calendar_id),
        );
        params
    }
}

impl TryFrom<&RemoteConfigParams> for OutlookRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let outlook_account = params
            .get("outlook_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: outlook_account"))?
            .to_string();

        let outlook_calendar_id = params
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
