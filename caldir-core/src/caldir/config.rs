use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::utils::tilde_expansion::expand_tilde;

const DEFAULT_CALDIR_PATH: &str = "~/caldir";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaldirConfig {
    pub calendar_dir: PathBuf,
}

impl Default for CaldirConfig {
    fn default() -> Self {
        Self {
            calendar_dir: PathBuf::from(DEFAULT_CALDIR_PATH),
        }
    }
}

impl CaldirConfig {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn calendar_dir(&self) -> PathBuf {
        expand_tilde(&self.calendar_dir)
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
    fn deserializes_user_config() {
        let dir = "/tmp/calendar";

        let toml = format!(r#"calendar_dir = "{dir}""#);

        let config = CaldirConfig::from_toml(&toml).unwrap();

        assert_eq!(config.calendar_dir, PathBuf::from(dir));
    }

    #[test]
    fn calendar_dir_expands_default() {
        let home = home::home_dir().unwrap();
        let config = CaldirConfig::default();

        assert_eq!(config.calendar_dir(), home.join("caldir"));
    }
}
