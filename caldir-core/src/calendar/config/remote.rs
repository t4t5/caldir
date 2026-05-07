use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarRemoteConfig {
    pub provider: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_and_flattened_params() {
        let toml_str = r#"
provider = "google"
google_calendar_id = "abc@group.calendar.google.com"
google_account = "user@gmail.com"
"#;

        let remote: CalendarRemoteConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(remote.provider, "google");
        assert_eq!(
            remote.params.get("google_account"),
            Some(&toml::Value::String("user@gmail.com".to_string()))
        );
        assert_eq!(
            remote.params.get("google_calendar_id"),
            Some(&toml::Value::String(
                "abc@group.calendar.google.com".to_string()
            ))
        );
    }

    #[test]
    fn parses_provider_with_no_params() {
        let remote: CalendarRemoteConfig = toml::from_str(r#"provider = "caldav""#).unwrap();

        assert_eq!(remote.provider, "caldav");
        assert!(remote.params.is_empty());
    }

    #[test]
    fn round_trip_preserves_provider_and_params() {
        let mut params = BTreeMap::new();
        params.insert(
            "google_account".to_string(),
            toml::Value::String("user@gmail.com".to_string()),
        );
        params.insert(
            "google_calendar_id".to_string(),
            toml::Value::String("abc@group.calendar.google.com".to_string()),
        );
        let remote = CalendarRemoteConfig {
            provider: "google".to_string(),
            params,
        };

        let serialized = toml::to_string(&remote).unwrap();
        let parsed: CalendarRemoteConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed, remote);
    }

    #[test]
    fn missing_provider_errors() {
        let result: Result<CalendarRemoteConfig, _> =
            toml::from_str(r#"google_account = "user@gmail.com""#);

        assert!(result.is_err());
    }
}
