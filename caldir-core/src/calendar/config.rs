mod error;
mod remote;

use remote::CalendarRemoteConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use error::CalendarConfigError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarConfig {
    name: Option<String>,
    color: Option<String>,
    read_only: Option<bool>,
    remote: Option<CalendarRemoteConfig>,
}

impl CalendarConfig {
    pub fn new(
        name: Option<String>,
        color: Option<String>,
        read_only: Option<bool>,
        remote: Option<CalendarRemoteConfig>,
    ) -> Self {
        Self {
            name,
            color,
            read_only,
            remote,
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), CalendarConfigError> {
        let contents = self.to_toml().map_err(CalendarConfigError::InvalidConfig)?;

        std::fs::write(path, contents)?;

        Ok(())
    }

    pub fn load_optional(path: &Path) -> Result<Option<Self>, CalendarConfigError> {
        if path.is_file() {
            let config = Self::load(path)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn load(path: &Path) -> Result<Self, CalendarConfigError> {
        let contents = std::fs::read_to_string(path)?;

        let config = Self::from_toml(&contents)
            .map_err(|e| CalendarConfigError::InvalidConfigFile(path.into(), e))?;

        Ok(config)
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
    use crate::{ProviderSlug, test_utils::test_calendar_config};

    #[test]
    fn write_saves_config_to_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        let config = test_calendar_config();

        config.write(&path).unwrap();

        let loaded = CalendarConfig::load(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn load_optional_returns_none_when_file_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.toml");

        let result = CalendarConfig::load_optional(&path).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn load_optional_returns_config_when_file_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        let config = test_calendar_config();
        config.write(&path).unwrap();

        let loaded = CalendarConfig::load_optional(&path).unwrap().unwrap();

        assert_eq!(loaded, config);
    }

    #[test]
    fn from_toml_parses_full_config_with_remote() {
        let toml_str = r##"
name = "Demo"
color = "#ac725e"
read_only = false

[remote]
provider = "hooli"
hooli_calendar_id = "abc@group.calendar.hooli.com"
hooli_account = "user@hmail.com"
"##;

        let config = CalendarConfig::from_toml(toml_str).unwrap();

        assert_eq!(config.name.as_deref(), Some("Demo"));
        assert_eq!(config.color.as_deref(), Some("#ac725e"));
        assert_eq!(config.read_only, Some(false));
        let remote = config.remote.expect("remote should be present");
        assert_eq!(remote.provider_slug.to_string(), "hooli");
        assert_eq!(
            remote.params.get("hooli_account"),
            Some(&toml::Value::String("user@hmail.com".to_string()))
        );
    }

    #[test]
    fn load_optional_errors_on_invalid_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "this is not = valid = toml").unwrap();

        let result = CalendarConfig::load_optional(&path);

        assert!(matches!(
            result,
            Err(CalendarConfigError::InvalidConfigFile(p, _)) if p == path
        ));
    }

    #[test]
    fn writes_full_config_with_remote_to_expected_toml() {
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "hooli_calendar_id".to_string(),
            toml::Value::String("abc@group.calendar.hooli.com".to_string()),
        );
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        let config = CalendarConfig::new(
            Some("Demo".to_string()),
            Some("#ac725e".to_string()),
            Some(false),
            Some(CalendarRemoteConfig {
                provider_slug: ProviderSlug::from("hooli"),
                params,
            }),
        );

        let serialized = config.to_toml().unwrap();

        let expected = r##"name = "Demo"
color = "#ac725e"
read_only = false

[remote]
provider = "hooli"
hooli_account = "user@hmail.com"
hooli_calendar_id = "abc@group.calendar.hooli.com"
"##;
        assert_eq!(serialized, expected);
    }
}
