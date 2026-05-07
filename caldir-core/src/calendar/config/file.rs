use std::path::PathBuf;

use super::CalendarConfig;
use super::error::CalendarConfigError;

pub struct CalendarConfigFile {
    path: PathBuf,
    config: CalendarConfig,
}

impl CalendarConfigFile {
    pub fn create(path: &PathBuf, config: CalendarConfig) -> Result<Self, CalendarConfigError> {
        let contents = config
            .to_toml()
            .map_err(CalendarConfigError::InvalidConfig)?;

        std::fs::write(path, contents)?;

        Ok(Self {
            path: path.clone(),
            config,
        })
    }

    pub fn load_optional(path: PathBuf) -> Result<Option<Self>, CalendarConfigError> {
        if path.is_file() {
            let config_file = CalendarConfigFile::load(path)?;
            Ok(Some(config_file))
        } else {
            Ok(None)
        }
    }

    pub fn load(path: PathBuf) -> Result<Self, CalendarConfigError> {
        let contents = std::fs::read_to_string(&path)?;

        let config = CalendarConfig::from_toml(&contents)
            .map_err(|e| CalendarConfigError::InvalidConfigFile(path.clone(), e))?;

        Ok(Self { path, config })
    }

    pub fn config(&self) -> &CalendarConfig {
        &self.config
    }
}
