//! Caldir root directory management.

use std::path::{Path, PathBuf};

use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use config::{Config, File};

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
    /// Where this caldir's config TOML lives. Saved-back writes target this
    /// path, so tests using tempdirs can never clobber the user's real
    /// `~/.config/caldir/config.toml`.
    config_path: PathBuf,
}

impl Caldir {
    /// Load a Caldir from the given config TOML path. If the file doesn't
    /// exist, a default one is created at that path.
    ///
    /// There's intentionally no zero-arg variant that resolves the config
    /// path via global state. CLI entry points compute it explicitly via
    /// `CaldirConfig::config_path()` so every read of the platform config
    /// directory is auditable; tests use [`Caldir::with_data_path`] instead.
    pub fn load(config_path: impl AsRef<Path>) -> CalDirResult<Self> {
        let config_path = config_path.as_ref().to_path_buf();

        if !config_path.exists() {
            CaldirConfig::create_default_config(&config_path)?;
        }

        let config: CaldirConfig = Config::builder()
            .add_source(File::from(config_path.clone()).required(false))
            .build()
            .map_err(|e| CalDirError::Config(e.to_string()))?
            .try_deserialize()
            .map_err(|e| CalDirError::Config(e.to_string()))?;

        Ok(Caldir {
            config,
            config_path,
        })
    }

    /// Construct a Caldir pointing at an explicit data directory, bypassing
    /// any config file. Useful for tests that want to operate on a tempdir.
    /// `set_default_calendar_if_unset` and similar mutating operations write
    /// a sidecar config file inside the data directory, keeping all writes
    /// confined to the tempdir.
    pub fn with_data_path(data_path: PathBuf) -> Self {
        let config_path = data_path.join("caldir.toml");
        Caldir {
            config: CaldirConfig {
                calendar_dir: data_path,
                ..CaldirConfig::default()
            },
            config_path,
        }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn data_path(&self) -> PathBuf {
        let full_path_str =
            shellexpand::tilde(&self.config.calendar_dir.to_string_lossy()).into_owned();

        PathBuf::from(full_path_str)
    }

    /// Returns the calendar directory path in display-friendly form,
    /// keeping `~` instead of expanding to the full home directory.
    pub fn display_path(&self) -> PathBuf {
        self.config.calendar_dir.clone()
    }

    /// Load a single calendar by slug, anchored at this caldir's data path.
    pub fn calendar(&self, slug: &str) -> CalDirResult<Calendar> {
        Calendar::load(slug, self.data_path())
    }

    /// Construct an in-memory calendar (not yet on disk) anchored at this
    /// caldir's data path. Used by the `connect` flow.
    pub fn new_calendar(
        &self,
        slug: &str,
        config: crate::calendar::config::CalendarConfig,
    ) -> Calendar {
        Calendar::new(slug, self.data_path(), config)
    }

    /// Generate a slug for a new calendar with the given display name that
    /// doesn't collide with any existing directory in this caldir.
    pub fn unique_slug_for(&self, name: Option<&str>) -> CalDirResult<String> {
        Calendar::unique_slug(name, &self.data_path())
    }

    /// Discover calendars by scanning calendar_dir for subdirectories.
    /// Every non-hidden directory is a calendar; `.caldir/config.toml`
    /// is optional and only carries metadata + remote sync settings.
    pub fn calendars(&self) -> Vec<Calendar> {
        let data_path = self.data_path();

        let Ok(entries) = std::fs::read_dir(&data_path) else {
            return Vec::new();
        };

        let mut calendars: Vec<Calendar> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter_map(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .filter(|name| !name.starts_with('.'))
                    .and_then(|name| Calendar::load(name, &data_path).ok())
            })
            .collect();

        calendars.sort_by(|a, b| a.slug.cmp(&b.slug));
        calendars
    }

    pub fn default_calendar(&self) -> Option<Calendar> {
        let name = self.config.default_calendar.as_ref()?;
        self.calendars().into_iter().find(|c| &c.slug == name)
    }

    /// Set the default calendar if one isn't already configured.
    /// Returns true if the default was set.
    pub fn set_default_calendar_if_unset(&mut self, slug: &str) -> CalDirResult<bool> {
        if self.config.default_calendar.is_some() {
            return Ok(false);
        }
        self.config.default_calendar = Some(slug.to_string());
        self.config.save_to(&self.config_path)?;
        Ok(true)
    }
}
