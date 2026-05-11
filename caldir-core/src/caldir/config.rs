mod error;
mod time_format;

use crate::{Reminder, utils::tilde_expansion::expand_tilde};
use error::CaldirConfigError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub(crate) use time_format::TimeFormat;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaldirConfig {
    #[serde(rename = "calendar_dir")] // preserved for backwards-compatibility
    data_dir: PathBuf,

    #[serde(default)]
    time_format: TimeFormat,

    #[serde(rename = "default_calendar", skip_serializing_if = "Option::is_none")]
    default_calendar_slug: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    default_reminders: Option<Vec<Reminder>>,
}

// Default config values (if empty file):
impl Default for CaldirConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("~/caldir"),
            time_format: TimeFormat::default(),
            default_calendar_slug: None,
            default_reminders: None,
        }
    }
}

impl CaldirConfig {
    #[cfg(test)]
    pub(crate) fn new(
        data_dir: PathBuf,
        time_format: TimeFormat,
        default_calendar_slug: Option<String>,
        default_reminders: Option<Vec<Reminder>>,
    ) -> Self {
        Self {
            data_dir,
            time_format,
            default_calendar_slug,
            default_reminders,
        }
    }

    pub fn load_or_default(path: &Path) -> Result<Self, CaldirConfigError> {
        if path.is_file() {
            let config = Self::load(path)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        expand_tilde(&self.data_dir)
    }

    pub fn default_calendar_slug(&self) -> Option<&str> {
        self.default_calendar_slug.as_deref()
    }

    pub fn write(&self, path: &Path) -> Result<(), CaldirConfigError> {
        let contents = self.to_toml().map_err(CaldirConfigError::InvalidConfig)?;

        std::fs::write(path, contents)?;

        Ok(())
    }

    fn load(path: &Path) -> Result<Self, CaldirConfigError> {
        let contents = std::fs::read_to_string(path)?;

        let config = Self::from_toml(&contents)
            .map_err(|e| CaldirConfigError::InvalidConfigFile(path.into(), e))?;

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

    #[test]
    fn default_has_expected_default_data_dir() {
        let home = home::home_dir().unwrap();
        let config = CaldirConfig::default();

        assert_eq!(config.data_dir(), home.join("caldir"));
    }

    #[test]
    fn load_or_default_uses_default_values_for_empty_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "").unwrap();

        let config = CaldirConfig::load_or_default(&path).unwrap();

        assert_eq!(config, CaldirConfig::default());
    }

    #[test]
    fn load_or_default_parses_user_file() {
        let data_dir = "/tmp/my-calendar";
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            format!(
                r#"
                calendar_dir = "{data_dir}"
                time_format = "12h"
                default_calendar = "personal"
                "#
            ),
        )
        .unwrap();

        let config = CaldirConfig::load_or_default(&path).unwrap();

        assert_eq!(config.data_dir, PathBuf::from(data_dir));
        assert_eq!(config.time_format, TimeFormat::H12);
        assert_eq!(config.default_calendar_slug.as_deref(), Some("personal"));
    }

    #[test]
    fn load_or_default_parses_default_reminders_as_human_durations() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
            default_reminders = [
                "30m",
                "1h",
            ]
            "#,
        )
        .unwrap();

        let config = CaldirConfig::load_or_default(&path).unwrap();

        assert_eq!(
            config.default_reminders,
            Some(vec![
                Reminder::minutes_before_start(30),
                Reminder::minutes_before_start(60),
            ])
        );
    }

    #[test]
    fn load_or_default_returns_default_on_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.toml");

        let config = CaldirConfig::load_or_default(&path).unwrap();

        assert_eq!(config, CaldirConfig::default());
    }

    #[test]
    fn load_or_default_errors_on_invalid_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("invalid.toml");
        std::fs::write(&path, "not a valid toml").unwrap();

        let result = CaldirConfig::load_or_default(&path);

        assert!(matches!(
            result.unwrap_err(),
            CaldirConfigError::InvalidConfigFile(_, _)
        ));
    }
}
