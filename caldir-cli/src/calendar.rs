use crate::caldir::Caldir;
use crate::config::CalendarConfig;
use crate::local_event::LocalEvent;
use crate::remote::Remote;
use anyhow::Result;

pub struct Calendar {
    pub name: String,
    pub config: CalendarConfig,
    pub caldir: Caldir,
}

impl Calendar {
    pub fn from(name: &str, caldir: &Caldir, config: &CalendarConfig) -> Self {
        Calendar {
            name: name.to_string(),
            caldir: caldir.clone(),
            config: config.clone(),
        }
    }

    pub fn remote(&self) -> Remote {
        Remote::from_calendar_config(&self.config)
    }

    fn data_path(&self) -> std::path::PathBuf {
        self.caldir.data_path().join(&self.name)
    }

    pub fn events(&self) -> Result<Vec<LocalEvent>> {
        let local_events = std::fs::read_dir(self.data_path())?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "ics"))
            .filter_map(|path| LocalEvent::from_file(path).ok())
            .collect();

        Ok(local_events)
    }
}
