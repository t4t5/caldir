mod error;

use crate::utils::tilde_expansion::expand_tilde;
use error::CaldirConfigError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Default config values:
const DEFAULT_DATA_DIR: &str = "~/caldir";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaldirConfig {
    #[serde(rename = "calendar_dir")] // preserved for backwards-compatibility
    data_dir: PathBuf,
}

impl Default for CaldirConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(DEFAULT_DATA_DIR),
        }
    }
}

impl CaldirConfig {
    pub(crate) fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub(crate) fn load(path: &PathBuf) -> Result<Self, CaldirConfigError> {
        let contents = std::fs::read_to_string(path)?;

        let config = Self::from_toml(&contents)
            .map_err(|e| CaldirConfigError::InvalidConfigFile(path.into(), e))?;

        Ok(config)
    }

    pub fn data_dir(&self) -> PathBuf {
        expand_tilde(&self.data_dir)
    }

    pub fn write(&self, path: &Path) -> Result<(), CaldirConfigError> {
        let contents = self.to_toml().map_err(CaldirConfigError::InvalidConfig)?;

        std::fs::write(path, contents)?;

        Ok(())
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
    fn empty_file_uses_default_values() {
        let config = CaldirConfig::from_toml("").unwrap();

        assert_eq!(config, CaldirConfig::default());
    }

    #[test]
    fn from_toml_parses_user_config() {
        let data_dir = "/tmp/calendar";

        let toml = format!(r#"calendar_dir = "{data_dir}""#);

        let config = CaldirConfig::from_toml(&toml).unwrap();

        assert_eq!(config.data_dir, PathBuf::from(data_dir));
    }

    #[test]
    fn default_has_default_data_dir() {
        let home = home::home_dir().unwrap();
        let config = CaldirConfig::default();

        assert_eq!(config.data_dir(), home.join("caldir"));
    }
}
