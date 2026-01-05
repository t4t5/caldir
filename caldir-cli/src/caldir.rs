use std::path::PathBuf;

use crate::calendar::{Calendar, Provider};
use crate::config::CaldirConfig;
use anyhow::Result;
use config::{Config, File};

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
}

impl Caldir {
    pub fn load() -> Result<Self> {
        let config_path = CaldirConfig::config_path()?;

        let config: CaldirConfig = Config::builder()
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
        self.config
            .calendars
            .iter()
            .map(|(name, entry)| {
                Calendar::from(name, self.clone(), Provider::from_name(&entry.provider))
            })
            .collect()
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.name == name)
    }
}
