//! Caldir root directory management.

use std::path::PathBuf;

use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use config::{Config, File};

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
}

impl Caldir {
    pub fn load() -> CalDirResult<Self> {
        let config_path = CaldirConfig::config_path()?;

        let config: CaldirConfig = Config::builder()
            .add_source(File::from(config_path).required(false))
            .build()
            .map_err(|e| CalDirError::Config(e.to_string()))?
            .try_deserialize()
            .map_err(|e| CalDirError::Config(e.to_string()))?;

        Ok(Caldir { config })
    }

    pub fn data_path(&self) -> PathBuf {
        let full_path_str =
            shellexpand::tilde(&self.config.calendar_dir.to_string_lossy()).into_owned();

        PathBuf::from(full_path_str)
    }

    /// Discover calendars by scanning calendar_dir for subdirectories
    /// with .caldir/config.toml files.
    pub fn calendars(&self) -> Vec<Calendar> {
        let data_path = self.data_path();

        let Ok(entries) = std::fs::read_dir(&data_path) else {
            return Vec::new();
        };

        let mut calendars: Vec<Calendar> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join(".caldir").exists())
            .filter_map(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|name| Calendar::load(name).ok())
            })
            .collect();

        calendars.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
        calendars
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.dir_name == name)
    }
}
