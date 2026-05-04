//! Calendar directory management.

mod cache;
mod calendar_event;
pub mod config;
mod events;
mod split;
mod state;
#[cfg(test)]
mod test_support;

pub use calendar_event::CalendarEvent;

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
    pub dir: PathBuf, // ~/caldir/{slug}
    pub config: CalendarConfig,
}

impl Calendar {
    /// Load a calendar at `caldir_dir/slug`
    /// (`caldir_dir` is `~/caldir` in production, a tempdir in tests).
    pub fn load(slug: &str, caldir_dir: &Path) -> CalDirResult<Self> {
        let dir = caldir_dir.join(slug);
        let config = CalendarConfig::load(&dir)?;

        Ok(Calendar {
            slug: slug.to_string(),
            dir,
            config,
        })
    }

    /// Construct an in-memory calendar without touching disk.
    /// Used by the `connect` flow when materializing a new calendar
    pub fn new(caldir_dir: &Path, config: &CalendarConfig) -> CalDirResult<Self> {
        let slug = Self::unique_slug(config.name.as_deref(), caldir_dir)?;
        let dir = caldir_dir.join(&slug);

        Ok(Calendar {
            slug,
            dir,
            config: config.clone(),
        })
    }

    pub fn dir(&self) -> &Path {
        self.dir.as_path()
    }

    pub fn state(&self) -> CalendarState {
        CalendarState::load(self.clone())
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(self.dir())
    }

    /// Get the account email for this calendar (from remote config)
    pub fn account_email(&self) -> Option<&str> {
        self.config.remote.as_ref()?.account_identifier()
    }

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<&Remote> {
        self.config.remote.as_ref()
    }

    fn base_slug_for(name: Option<&str>) -> String {
        name.map(slugify).unwrap_or_else(|| "calendar".to_string())
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    fn unique_slug(name: Option<&str>, caldir_dir: &Path) -> CalDirResult<String> {
        let base = Self::base_slug_for(name);

        if !caldir_dir.join(&base).exists() {
            return Ok(base);
        }

        for n in 2..=100 {
            let suffixed = format!("{}-{}", base, n);
            if !caldir_dir.join(&suffixed).exists() {
                return Ok(suffixed);
            }
        }

        Err(CalDirError::Config(format!(
            "Too many calendar name collisions for '{}'",
            base
        )))
    }
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_slug_returns_base_when_no_collision() {
        let tmp = tempfile::tempdir().unwrap();
        let slug = Calendar::unique_slug(Some("Personal"), tmp.path()).unwrap();
        assert_eq!(slug, "personal");
    }

    #[test]
    fn unique_slug_falls_back_to_calendar_when_name_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let slug = Calendar::unique_slug(None, tmp.path()).unwrap();
        assert_eq!(slug, "calendar");
    }

    #[test]
    fn unique_slug_suffixes_when_base_is_taken() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("personal")).unwrap();

        let slug = Calendar::unique_slug(Some("Personal"), tmp.path()).unwrap();
        assert_eq!(slug, "personal-2");

        std::fs::create_dir_all(tmp.path().join("personal-2")).unwrap();
        let slug = Calendar::unique_slug(Some("Personal"), tmp.path()).unwrap();
        assert_eq!(slug, "personal-3");
    }

    #[test]
    fn unique_slug_errors_when_collisions_exhausted() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("personal")).unwrap();
        for n in 2..=100 {
            std::fs::create_dir_all(tmp.path().join(format!("personal-{n}"))).unwrap();
        }

        let Err(err) = Calendar::unique_slug(Some("Personal"), tmp.path()) else {
            panic!("expected collision exhaustion to fail");
        };
        assert!(matches!(err, CalDirError::Config(_)));
    }
}
