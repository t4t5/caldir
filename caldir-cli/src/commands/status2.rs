use std::path::PathBuf;

use anyhow::Result;
use config::{Config, File};

use crate::config::CaldirConfig;

pub struct Provider(String);

pub struct Calendar {
    name: String,
    caldir: Caldir,
    provider: Provider,
}

impl Calendar {
    pub fn from(name: &str, caldir: &Caldir, provider: Provider) -> Result<Self> {
        Ok(Calendar {
            name: name.to_string(),
            caldir: caldir.clone(),
            provider,
        })
    }
}

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
}

impl Caldir {
    pub fn load() -> Result<Self> {
        let config: CaldirConfig = Config::builder()
            .add_source(File::from(CaldirConfig::config_path()?).required(false))
            .build()?
            .try_deserialize()?;

        Ok(Caldir { config })
    }

    fn path(&self) -> PathBuf {
        PathBuf::from(shellexpand::tilde(&self.config.calendar_dir.to_string_lossy()).into_owned())
    }

    pub fn calendars(&self) -> Vec<Calendar> {
        self.config
            .calendars
            .iter()
            .map(|(name, entry)| {
                Calendar::from(name, self, Provider(entry.provider.clone())).unwrap()
            })
            .collect()
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.name == name)
    }
}

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;

    println!("Path: {:?}", caldir.path());
    println!("Calendars: {:?}", caldir.calendars().len());

    Ok(())
}
