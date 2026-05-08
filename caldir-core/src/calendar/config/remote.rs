use serde::{Deserialize, Serialize};

use crate::{ProviderSlug, RemoteConfig};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarRemoteConfig {
    #[serde(rename = "provider")]
    provider_slug: ProviderSlug,
    #[serde(flatten)]
    remote_config: RemoteConfig,
}

impl CalendarRemoteConfig {
    pub fn new(provider_slug: ProviderSlug, remote_config: RemoteConfig) -> Self {
        Self {
            provider_slug,
            remote_config,
        }
    }

    pub fn provider_slug(&self) -> &ProviderSlug {
        &self.provider_slug
    }

    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.params().get(key)
    }

    fn params(&self) -> &RemoteConfig {
        &self.remote_config
    }

    fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

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

        let remote_config = CalendarRemoteConfig::from_toml(toml_str).unwrap();

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
        let remote_config = CalendarRemoteConfig::from_toml(r#"provider = "caldav""#).unwrap();

        assert_eq!(remote_config.provider_slug.to_string(), "caldav");
        assert!(remote_config.params().is_empty());
    }

    #[test]
    fn round_trip_preserves_provider_and_params() {
        let mut remote_config = RemoteConfig::new();
        remote_config.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        remote_config.insert(
            "hooli_calendar_id".to_string(),
            toml::Value::String("abc@group.calendar.hooli.com".to_string()),
        );

        let remote = CalendarRemoteConfig::new(ProviderSlug::from("hooli"), remote_config);

        let serialized = remote.to_toml().unwrap();
        let parsed = CalendarRemoteConfig::from_toml(&serialized).unwrap();

        assert_eq!(parsed, remote);
    }

    #[test]
    fn missing_provider_errors() {
        let result = CalendarRemoteConfig::from_toml(r#"hooli_account = "user@hmail.com""#);

        assert!(result.is_err());
    }
}
