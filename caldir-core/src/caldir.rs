//! Master struct. Everything else flows from here

use std::path::{Path, PathBuf};

use crate::caldir_config::CaldirConfig;
use crate::caldir_environment::CaldirEnvironment;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;
use crate::utils::expand_tilde;

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
    config_path: Option<PathBuf>,
    providers: Vec<Provider>,
}

impl Caldir {
    /// Load from the current process environment.
    pub fn load() -> CalDirResult<Self> {
        CaldirEnvironment::from_process()?.load()
    }

    /// Construct a Caldir directly from a config and provider list.
    pub fn new(config: CaldirConfig, providers: Vec<Provider>) -> Self {
        Caldir {
            config,
            config_path: None,
            providers,
        }
    }

    /// Set the path that `save_config` writes to.
    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        match &self.config_path {
            Some(path) => self.config.save_to(path),
            None => Ok(()),
        }
    }

    pub fn data_path(&self) -> PathBuf {
        expand_tilde(&self.config.calendar_dir)
    }

    /// Returns the calendar directory path in display-friendly form,
    /// keeping `~` instead of expanding to the full home directory.
    pub fn display_path(&self) -> PathBuf {
        self.config.calendar_dir.clone()
    }

    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|provider| provider.name().to_string())
            .collect()
    }

    pub fn provider(&self, name: &str) -> CalDirResult<Provider> {
        self.providers
            .iter()
            .find(|provider| provider.name() == name)
            .cloned()
            .ok_or_else(|| CalDirError::ProviderNotInstalled(name.to_string()))
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
    pub fn calendars(&self) -> CalDirResult<Vec<Calendar>> {
        let data_path = self.data_path();

        let entries = match std::fs::read_dir(&data_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };

        let mut calendars = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }

            calendars.push(Calendar::load(name, &data_path)?);
        }

        calendars.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(calendars)
    }

    pub fn default_calendar(&self) -> CalDirResult<Option<Calendar>> {
        let Some(name) = self.config.default_calendar.as_ref() else {
            return Ok(None);
        };
        Ok(self.calendars()?.into_iter().find(|c| &c.slug == name))
    }

    /// Set the default calendar if one isn't already configured.
    /// Returns true if the default was set.
    pub fn set_default_calendar_if_unset(&mut self, slug: &str) -> bool {
        if self.config.default_calendar.is_some() {
            return false;
        }
        self.config.default_calendar = Some(slug.to_string());
        true
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use tempfile::TempDir;

    /// Build a fresh Caldir rooted at a tempdir, with no providers and no
    /// config save path. The TempDir is returned alongside so it stays alive
    /// for the test's lifetime — drop it and the calendar dir disappears.
    pub fn mock_caldir() -> (TempDir, Caldir) {
        let tmp = tempfile::tempdir().unwrap();
        let calendar_dir = tmp.path().join("caldir");
        std::fs::create_dir_all(&calendar_dir).unwrap();
        let caldir = Caldir::new(
            CaldirConfig {
                calendar_dir,
                ..CaldirConfig::default()
            },
            Vec::new(),
        );
        (tmp, caldir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::mock_caldir;

    #[test]
    fn caldir_with_config_path_saves_to_that_path() {
        let (tmp, caldir) = mock_caldir();
        let config_path = tmp.path().join("config.toml");
        let mut caldir = caldir.with_config_path(config_path.clone());

        assert_eq!(caldir.config_path(), Some(config_path.as_path()));
        assert!(caldir.set_default_calendar_if_unset("work"));
        caldir.save_config().unwrap();

        let contents = std::fs::read_to_string(config_path).unwrap();
        assert!(contents.contains("default_calendar = \"work\""));
    }

    #[test]
    fn calendars_returns_error_for_invalid_calendar_config() {
        let (_tmp, caldir) = mock_caldir();
        let config_dir = caldir.data_path().join("work/.caldir");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "remote =").unwrap();

        let Err(error) = caldir.calendars() else {
            panic!("expected invalid calendar config to fail");
        };

        assert!(matches!(error, CalDirError::Config(_)));
    }

    #[test]
    fn calendars_returns_empty_when_data_path_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let caldir = Caldir::new(
            CaldirConfig {
                calendar_dir: tmp.path().join("missing"),
                ..CaldirConfig::default()
            },
            Vec::new(),
        );

        assert!(caldir.calendars().unwrap().is_empty());
    }
}
