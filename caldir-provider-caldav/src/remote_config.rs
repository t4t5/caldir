//! CalDAV-specific remote configuration.

use anyhow::Result;
use caldir_core::RemoteConfigParams;
use serde::{Deserialize, Serialize};

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

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "caldav_account".to_string(),
            toml::Value::String(self.caldav_account),
        );
        params.insert(
            "caldav_calendar_url".to_string(),
            toml::Value::String(self.caldav_calendar_url),
        );
        params
    }
}

impl TryFrom<&RemoteConfigParams> for CaldavRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let caldav_account = params
            .get("caldav_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: caldav_account"))?
            .to_string();

        let caldav_calendar_url = params
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_remote_config_params_round_trips() {
        let original =
            CaldavRemoteConfig::new("me@fastmail.com", "https://caldav.fastmail.com/dav/cal/1/");
        let params = original.clone().into_remote_config_params();

        let restored = CaldavRemoteConfig::try_from(&params).unwrap();

        assert_eq!(restored.caldav_account, original.caldav_account);
        assert_eq!(restored.caldav_calendar_url, original.caldav_calendar_url);
    }

    #[test]
    fn try_from_missing_account_errors() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "caldav_calendar_url".to_string(),
            toml::Value::String("https://example/cal/".to_string()),
        );

        let err = CaldavRemoteConfig::try_from(&params).unwrap_err();
        assert!(err.to_string().contains("caldav_account"));
    }

    #[test]
    fn try_from_missing_url_errors() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "caldav_account".to_string(),
            toml::Value::String("me@example".to_string()),
        );

        let err = CaldavRemoteConfig::try_from(&params).unwrap_err();
        assert!(err.to_string().contains("caldav_calendar_url"));
    }
}
