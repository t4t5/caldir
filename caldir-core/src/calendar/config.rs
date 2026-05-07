mod error;
mod file;

use serde::{Deserialize, Serialize};

pub use error::CalendarConfigError;
pub use file::CalendarConfigFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    name: Option<String>,
    color: Option<String>,
    read_only: Option<bool>,
    // pub remote: Option<Remote>,
}

impl CalendarConfig {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}
