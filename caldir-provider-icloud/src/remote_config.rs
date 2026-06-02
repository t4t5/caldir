//! iCloud-specific remote configuration.

use anyhow::Result;
use caldir_core::RemoteConfigParams;
use serde::{Deserialize, Serialize};

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

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "icloud_account".to_string(),
            toml::Value::String(self.icloud_account),
        );
        params.insert(
            "icloud_calendar_url".to_string(),
            toml::Value::String(self.icloud_calendar_url),
        );
        params
    }
}

impl TryFrom<&RemoteConfigParams> for ICloudRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let icloud_account = params
            .get("icloud_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: icloud_account"))?
            .to_string();

        let icloud_calendar_url = params
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_remote_config_params_round_trips() {
        let original = ICloudRemoteConfig::new(
            "me@icloud.com",
            "https://p01-caldav.icloud.com/123/calendars/home/",
        );
        let params = original.clone().into_remote_config_params();

        let restored = ICloudRemoteConfig::try_from(&params).unwrap();

        assert_eq!(restored.icloud_account, original.icloud_account);
        assert_eq!(restored.icloud_calendar_url, original.icloud_calendar_url);
    }

    #[test]
    fn try_from_missing_account_errors() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "icloud_calendar_url".to_string(),
            toml::Value::String("https://example/cal/".to_string()),
        );

        let err = ICloudRemoteConfig::try_from(&params).unwrap_err();
        assert!(err.to_string().contains("icloud_account"));
    }

    #[test]
    fn try_from_missing_url_errors() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "icloud_account".to_string(),
            toml::Value::String("me@icloud.com".to_string()),
        );

        let err = ICloudRemoteConfig::try_from(&params).unwrap_err();
        assert!(err.to_string().contains("icloud_calendar_url"));
    }
}
