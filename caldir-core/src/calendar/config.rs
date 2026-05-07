mod error;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use error::CalendarConfigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    name: Option<String>,
    color: Option<String>,
    read_only: Option<bool>,
    // pub remote: Option<Remote>,
}

impl CalendarConfig {
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

    fn load(path: &Path) -> Result<Self, CalendarConfigError> {
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

    fn sample_toml() -> &'static str {
        "name = \"Work\"\ncolor = \"#0b8043\"\nread_only = false\n"
    }

    #[test]
    fn write_saves_config_to_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        let config = CalendarConfig::from_toml(sample_toml()).unwrap();

        config.write(&path).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, sample_toml());
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
        std::fs::write(&path, sample_toml()).unwrap();

        let loaded = CalendarConfig::load_optional(&path).unwrap().unwrap();

        assert_eq!(loaded.to_toml().unwrap(), sample_toml());
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
}
