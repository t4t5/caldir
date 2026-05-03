//! Calendar directory management.

mod cache;
mod calendar_event;
pub mod config;
mod events;
mod split;
mod state;
#[cfg(test)]
mod test_support;

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::calendar::config::CalendarConfig;
use crate::calendar::state::CalendarState;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::Remote;
use crate::utils::slugify;

#[derive(Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub slug: String,
    pub data_path: PathBuf, // ~/caldir/{slug}
    pub config: CalendarConfig,
}

impl Calendar {
    fn base_slug_for(name: Option<&str>) -> String {
        name.map(slugify).unwrap_or_else(|| "calendar".to_string())
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    pub fn unique_slug(name: Option<&str>, caldir_data_path: &Path) -> CalDirResult<String> {
        let base = Self::base_slug_for(name);

        if !caldir_data_path.join(&base).exists() {
            return Ok(base);
        }

        for n in 2..=100 {
            let suffixed = format!("{}-{}", base, n);
            if !caldir_data_path.join(&suffixed).exists() {
                return Ok(suffixed);
            }
        }

        Err(CalDirError::Config(format!(
            "Too many calendar name collisions for '{}'",
            base
        )))
    }

    /// Load a calendar at `caldir_data_path/slug`
    /// (caldir_data_path` is `~/caldir` in production, a tempdir in tests).
    pub fn load(slug: &str, caldir_data_path: impl AsRef<Path>) -> CalDirResult<Self> {
        let data_path = caldir_data_path.as_ref().join(slug);
        let config = CalendarConfig::load(&data_path)?;

        Ok(Calendar {
            slug: slug.to_string(),
            data_path,
            config,
        })
    }

    /// Construct an in-memory calendar without touching disk.
    /// Used by the `connect` flow when materializing a new calendar
    /// from a remote config before saving it.
    pub fn new(slug: &str, caldir_data_path: impl AsRef<Path>, config: CalendarConfig) -> Self {
        Calendar {
            slug: slug.to_string(),
            data_path: caldir_data_path.as_ref().join(slug),
            config,
        }
    }

    pub fn data_path(&self) -> &Path {
        self.data_path.as_path()
    }

    pub fn state(&self) -> CalendarState {
        CalendarState::load(self.clone())
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(self.data_path())
    }

    /// Get the account email for this calendar (from remote config)
    pub fn account_email(&self) -> Option<&str> {
        self.config.remote.as_ref()?.account_identifier()
    }

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<&Remote> {
        self.config.remote.as_ref()
    }
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.slug)
    }
}
