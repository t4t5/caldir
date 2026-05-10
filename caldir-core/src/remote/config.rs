mod params;

use crate::ProviderSlug;
use serde::{Deserialize, Serialize};

pub use params::RemoteConfigParams;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteConfig {
    #[serde(rename = "provider")]
    provider_slug: ProviderSlug,
    #[serde(flatten)]
    params: RemoteConfigParams,
}

/// A remote config is always part of a calendar config
/// so it doesn't need its own ::load and ::write methods
impl RemoteConfig {
    pub fn new(provider_slug: ProviderSlug, params: RemoteConfigParams) -> Self {
        Self {
            provider_slug,
            params,
        }
    }

    pub fn provider_slug(&self) -> &ProviderSlug {
        &self.provider_slug
    }

    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.params().get(key)
    }

    pub(crate) fn params(&self) -> &RemoteConfigParams {
        &self.params
    }

    #[cfg(test)]
    fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    #[cfg(test)]
    fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_and_flattened_params() {
        let toml_str = r#"
provider = "hooli"
hooli_calendar_id = "abc@group.calendar.hooli.com"
hooli_account = "user@hmail.com"
"#;

        let remote_config = RemoteConfig::from_toml(toml_str).unwrap();

        assert_eq!(remote_config.provider_slug.to_string(), "hooli");
        assert_eq!(
            remote_config.get("hooli_account"),
            Some(&toml::Value::String("user@hmail.com".to_string()))
        );
        assert_eq!(
            remote_config.get("hooli_calendar_id"),
            Some(&toml::Value::String(
                "abc@group.calendar.hooli.com".to_string()
            ))
        );
    }

    #[test]
    fn parses_provider_with_no_params() {
        let remote_config = RemoteConfig::from_toml(r#"provider = "caldav""#).unwrap();

        assert_eq!(remote_config.provider_slug.to_string(), "caldav");
        assert!(remote_config.params().is_empty());
    }

    #[test]
    fn round_trip_preserves_provider_and_params() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        params.insert(
            "hooli_calendar_id".to_string(),
            toml::Value::String("abc@group.calendar.hooli.com".to_string()),
        );

        let remote = RemoteConfig::new(ProviderSlug::from("hooli"), params);

        let serialized = remote.to_toml().unwrap();
        let parsed = RemoteConfig::from_toml(&serialized).unwrap();

        assert_eq!(parsed, remote);
    }

    #[test]
    fn missing_provider_errors() {
        let result = RemoteConfig::from_toml(r#"hooli_account = "user@hmail.com""#);

        assert!(result.is_err());
    }
}
