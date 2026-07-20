mod error;

use crate::remote::RemoteConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) use error::CalendarConfigError;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CalendarConfig {
    name: Option<String>,
    color: Option<String>,
    read_only: Option<bool>,

    #[serde(rename = "remote")]
    remote_config: Option<RemoteConfig>,
}

impl CalendarConfig {
    pub fn new(
        name: Option<String>,
        color: Option<String>,
        read_only: Option<bool>,
        remote_config: Option<RemoteConfig>,
    ) -> Self {
        Self {
            name,
            color,
            read_only,
            remote_config,
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

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub fn set_color(&mut self, color: Option<String>) {
        self.color = color;
    }

    pub fn read_only(&self) -> Option<bool> {
        self.read_only
    }

    fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    pub fn remote_config(&self) -> Option<&RemoteConfig> {
        self.remote_config.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn set_remote(&mut self, remote_config: RemoteConfig) {
        self.remote_config = Some(remote_config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_calendar_config;
    use crate::{ProviderSlug, RemoteConfig, RemoteConfigParams};

    #[test]
    fn set_name_only_updates_name() {
        let mut config = test_calendar_config();
        let color = config.color.clone();
        let read_only = config.read_only;
        let remote_config = config.remote_config.clone();

        config.set_name(Some("Renamed".to_string()));

        assert_eq!(config.name(), Some("Renamed"));
        assert_eq!(config.color, color);
        assert_eq!(config.read_only, read_only);
        assert_eq!(config.remote_config, remote_config);
    }

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

        let remote_config = config.remote_config.expect("remote should be present");
        assert_eq!(remote_config.provider_slug().to_string(), "hooli");
        assert_eq!(
            remote_config.get("hooli_account"),
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
        let mut params = RemoteConfigParams::new();

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
            Some(RemoteConfig::new(ProviderSlug::from("hooli"), params)),
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

    #[test]
    fn set_remote_sets_remote_config() {
        let mut config = test_calendar_config();
        assert!(config.remote_config().is_none());

        let remote = RemoteConfig::new(ProviderSlug::from("hooli"), RemoteConfigParams::new());
        config.set_remote(remote.clone());

        assert_eq!(config.remote_config(), Some(&remote));
    }

    #[test]
    fn set_remote_overwrites_existing_remote_config() {
        let mut config = test_calendar_config();
        config.set_remote(RemoteConfig::new(
            ProviderSlug::from("hooli"),
            RemoteConfigParams::new(),
        ));

        let new_remote = RemoteConfig::new(ProviderSlug::from("aviato"), RemoteConfigParams::new());
        config.set_remote(new_remote.clone());

        assert_eq!(config.remote_config(), Some(&new_remote));
    }
}
