mod error;
mod time_format;

use crate::{Reminder, utils::expand_tilde};
pub(crate) use error::CaldirConfigError;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
pub use time_format::TimeFormat;

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

impl Display for CaldirConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let toml = self.to_toml().map_err(|_| std::fmt::Error)?;
        write!(f, "{toml}")
    }
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
    pub fn new(
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

    pub fn time_format(&self) -> TimeFormat {
        self.time_format
    }

    pub fn default_calendar_slug(&self) -> Option<&str> {
        self.default_calendar_slug.as_deref()
    }

    pub fn set_default_calendar_slug(&mut self, slug: Option<String>) {
        self.default_calendar_slug = slug;
    }

    pub fn default_reminders(&self) -> Option<Vec<Reminder>> {
        self.default_reminders.clone()
    }

    pub fn set_data_dir(&mut self, path: std::path::PathBuf) {
        self.data_dir = path;
    }

    pub fn set_time_format(&mut self, time_format: TimeFormat) {
        self.time_format = time_format;
    }

    pub fn set_default_reminders(&mut self, reminders: Option<Vec<Reminder>>) {
        self.default_reminders = reminders;
    }

    pub fn write(&self, path: &Path) -> Result<(), CaldirConfigError> {
        let contents = self.to_toml().map_err(CaldirConfigError::InvalidConfig)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, contents)?;

        Ok(())
    }

    /// Caldir config directory:
    /// - Linux/BSD: `$XDG_CONFIG_HOME/caldir` or `~/.config/caldir`
    /// - macOS: `~/.config/caldir`
    /// - Windows: `%APPDATA%\caldir`
    pub fn default_system_config_path() -> Result<PathBuf, CaldirConfigError> {
        let dir = Self::platform_config_dir()?.join("caldir");

        Ok(dir.join("config.toml"))
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

    fn platform_config_dir() -> Result<PathBuf, CaldirConfigError> {
        crate::utils::paths::platform_config_dir().ok_or(CaldirConfigError::UnknownConfigDirectory)
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
            Some(vec![Reminder::from_minutes(30), Reminder::from_minutes(60),])
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

    #[test]
    #[cfg(target_os = "macos")]
    fn default_system_path_uses_home_dot_config_on_macos() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let path = CaldirConfig::default_system_config_path().unwrap();
        assert_eq!(path, home.join(".config/caldir/config.toml"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn default_system_path_uses_xdg_config_dir_on_linux() {
        let expected_parent = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"));

        let path = CaldirConfig::default_system_config_path().unwrap();

        assert_eq!(path, expected_parent.join("caldir/config.toml"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn default_system_path_uses_appdata_on_windows() {
        let appdata = PathBuf::from(std::env::var("APPDATA").unwrap());
        let path = CaldirConfig::default_system_config_path().unwrap();
        assert_eq!(path, appdata.join("caldir/config.toml"));
    }

    #[test]
    fn default_system_config_path_is_dot_config_caldir_config_toml() {
        let path = CaldirConfig::default_system_config_path().unwrap();
        let expected_path = expand_tilde(&PathBuf::from("~/.config/caldir/config.toml"));

        assert_eq!(path, expected_path);
    }

    #[test]
    fn write_then_load_round_trips_through_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");

        let config = CaldirConfig::new(
            PathBuf::from("/tmp/round-trip"),
            TimeFormat::H12,
            Some("personal".to_string()),
            Some(vec![Reminder::from_minutes(15)]),
        );

        config.write(&path).unwrap();

        let reloaded = CaldirConfig::load_or_default(&path).unwrap();
        assert_eq!(reloaded, config);
    }

    #[test]
    fn write_creates_missing_parent_directories() {
        // The system config path is ~/.config/caldir/config.toml — and on a
        // fresh machine the `caldir/` directory doesn't exist yet. write()
        // needs to be tolerant of that, otherwise first-run `caldir connect`
        // can't persist its choice of default_calendar.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("dir").join("config.toml");

        let config = CaldirConfig::default();
        config.write(&path).unwrap();

        assert!(path.is_file(), "config file should exist after write");
    }
}
