//! Webcal-specific remote configuration.

use anyhow::{Context, Result};
use caldir_core::RemoteConfigParams;
use serde::{Deserialize, Serialize};

/// Strongly-typed remote configuration for webcal subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcalRemoteConfig {
    pub webcal_url: String,
}

impl WebcalRemoteConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            webcal_url: url.into(),
        }
    }

    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "webcal_url".to_string(),
            toml::Value::String(self.webcal_url),
        );
        params
    }
}

impl TryFrom<&RemoteConfigParams> for WebcalRemoteConfig {
    type Error = anyhow::Error;

    fn try_from(params: &RemoteConfigParams) -> Result<Self> {
        let webcal_url = params
            .get("webcal_url")
            .context("Webcal remote configuration is missing required field `webcal_url`")?
            .as_str()
            .context("Webcal remote configuration field `webcal_url` must be a string")?
            .to_string();

        Ok(Self { webcal_url })
    }
}
