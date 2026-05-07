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
