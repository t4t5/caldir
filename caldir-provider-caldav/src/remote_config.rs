//! CalDAV-specific remote configuration.

use anyhow::{Context, Result};
use caldir_core::RemoteConfigParams;
use caldir_core::provider::PasswordCommand;
use serde::{Deserialize, Serialize};

/// Strongly-typed remote configuration for generic CalDAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaldavRemoteConfig {
    /// `user@host`; keys the stored session. Optional with `caldav_password_command`.
    pub caldav_account: Option<String>,
    pub caldav_calendar_url: String,
    pub caldav_email: Option<String>,
    /// Login for `caldav_password_command`.
    pub caldav_username: Option<String>,
    /// Prints the password; replaces the stored session.
    pub caldav_password_command: Option<PasswordCommand>,
}

impl CaldavRemoteConfig {
    pub fn new(
        account: impl Into<String>,
        calendar_url: impl Into<String>,
        email: Option<String>,
    ) -> Self {
        Self {
            caldav_account: Some(account.into()),
            caldav_calendar_url: calendar_url.into(),
            caldav_email: email,
            caldav_username: None,
            caldav_password_command: None,
        }
    }

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        if let Some(account) = self.caldav_account {
            params.insert("caldav_account".to_string(), toml::Value::String(account));
        }
        params.insert(
            "caldav_calendar_url".to_string(),
            toml::Value::String(self.caldav_calendar_url),
        );
        if let Some(email) = self.caldav_email {
            params.insert("caldav_email".to_string(), toml::Value::String(email));
        }
        params
    }
}

impl TryFrom<&RemoteConfigParams> for CaldavRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let caldav_account = optional_str(params, "caldav_account")?;

        let caldav_calendar_url = params
            .get("caldav_calendar_url")
            .context("CalDAV remote configuration is missing required field `caldav_calendar_url`")?
            .as_str()
            .context("CalDAV remote configuration field `caldav_calendar_url` must be a string")?
            .to_string();

        let caldav_email = params
            .get("caldav_email")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let caldav_username = optional_str(params, "caldav_username")?;

        let caldav_password_command = params
            .get("caldav_password_command")
            .map(|v| v.clone().try_into())
            .transpose()
            .context("CalDAV remote configuration field `caldav_password_command` must be a non-empty list of strings")?;

        Ok(Self {
            caldav_account,
            caldav_calendar_url,
            caldav_email,
            caldav_username,
            caldav_password_command,
        })
    }
}

fn optional_str(params: &RemoteConfigParams, key: &str) -> Result<Option<String>> {
    params
        .get(key)
        .map(|v| {
            v.as_str().map(str::to_string).with_context(|| {
                format!("CalDAV remote configuration field `{key}` must be a string")
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_remote_config_params_round_trips() {
        let original = CaldavRemoteConfig::new(
            "me@fastmail.com",
            "https://caldav.fastmail.com/dav/cal/1/",
            Some("me@example.com".to_string()),
        );
        let params = original.clone().into_remote_config_params();

        let restored = CaldavRemoteConfig::try_from(&params).unwrap();

        assert_eq!(restored.caldav_account, original.caldav_account);
        assert_eq!(restored.caldav_calendar_url, original.caldav_calendar_url);
        assert_eq!(restored.caldav_email, original.caldav_email);
    }

    #[test]
    fn try_from_reads_username_and_password_command() {
        let params: RemoteConfigParams = toml::from_str(
            r#"
caldav_calendar_url = "https://example/cal/"
caldav_username = "alice@example.com"
caldav_password_command = ["pass", "show", "caldav"]
"#,
        )
        .unwrap();

        let config = CaldavRemoteConfig::try_from(&params).unwrap();
        assert_eq!(config.caldav_account, None);
        assert_eq!(config.caldav_username.as_deref(), Some("alice@example.com"));
        assert!(config.caldav_password_command.is_some());
    }

    #[test]
    fn try_from_rejects_string_password_command() {
        let params: RemoteConfigParams = toml::from_str(
            r#"
caldav_calendar_url = "https://example/cal/"
caldav_password_command = "pass show caldav"
"#,
        )
        .unwrap();

        let err = CaldavRemoteConfig::try_from(&params).unwrap_err();
        assert!(err.to_string().contains("caldav_password_command"));
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
