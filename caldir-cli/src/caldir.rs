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

    pub fn calendars(&self) -> Vec<Calendar> {
        let mut calendars: Vec<_> = self
            .config
            .calendars
            .iter()
            .map(|(name, config)| Calendar::from(name, self, config))
            .collect();

        calendars.sort_by(|a, b| a.name.cmp(&b.name));
        calendars
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.name == name)
    }
}
