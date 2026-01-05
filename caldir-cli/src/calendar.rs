use crate::{caldir::Caldir, config::CalendarConfig, remote::Remote};

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
}
