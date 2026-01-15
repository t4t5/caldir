use std::path::PathBuf;

use crate::calendar::Calendar;
use crate::config::GlobalConfig;
use anyhow::Result;
use config::{Config, File};

#[derive(Clone)]
pub struct Caldir {
    config: GlobalConfig,
}

impl Caldir {
    pub fn load() -> Result<Self> {
        let config_path = GlobalConfig::config_path()?;

        let config: GlobalConfig = Config::builder()
            .add_source(File::from(config_path).required(false))
            .build()?
            .try_deserialize()?;

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

        let mut calendars: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter(|path| path.join(".caldir").exists())
            .filter_map(|path| Calendar::load(&path).ok())
            .collect();

        calendars.sort_by(|a, b| a.name.cmp(&b.name));
        calendars
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.name == name)
    }
}
