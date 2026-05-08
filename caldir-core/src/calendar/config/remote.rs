use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ProviderSlug;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarRemoteConfig {
    #[serde(rename = "provider")]
    pub provider_slug: ProviderSlug,
    #[serde(flatten)]
    pub params: BTreeMap<String, toml::Value>,
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

        let remote: CalendarRemoteConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(remote.provider_slug.to_string(), "hooli");
        assert_eq!(
            remote.params.get("hooli_account"),
            Some(&toml::Value::String("user@hmail.com".to_string()))
        );
        assert_eq!(
            remote.params.get("hooli_calendar_id"),
            Some(&toml::Value::String(
                "abc@group.calendar.hooli.com".to_string()
            ))
        );
    }

    #[test]
    fn parses_provider_with_no_params() {
        let remote: CalendarRemoteConfig = toml::from_str(r#"provider = "caldav""#).unwrap();

        assert_eq!(remote.provider_slug.to_string(), "caldav");
        assert!(remote.params.is_empty());
    }

    #[test]
    fn round_trip_preserves_provider_and_params() {
        let mut params = BTreeMap::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        params.insert(
            "hooli_calendar_id".to_string(),
            toml::Value::String("abc@group.calendar.hooli.com".to_string()),
        );
        let remote = CalendarRemoteConfig {
            provider_slug: ProviderSlug::from("hooli"),
            params,
        };

        let serialized = toml::to_string(&remote).unwrap();
        let parsed: CalendarRemoteConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed, remote);
    }

    #[test]
    fn missing_provider_errors() {
        let result: Result<CalendarRemoteConfig, _> =
            toml::from_str(r#"hooli_account = "user@hmail.com""#);

        assert!(result.is_err());
    }
}
