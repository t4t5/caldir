//! Master struct. Everything else flows from here.

use std::path::PathBuf;

use crate::caldir_settings::CaldirSettings;
use crate::calendar::Calendar;
use crate::error::CalDirResult;
use crate::remote::provider::Provider;

pub struct Caldir {
    settings: CaldirSettings,
}

impl Caldir {
    /// Load from the current process environment.
    pub fn load() -> CalDirResult<Self> {
        Ok(Self::new(CaldirSettings::load()?))
    }

    /// Construct a Caldir from resolved settings.
    pub fn new(settings: CaldirSettings) -> Self {
        Caldir { settings }
    }

    pub fn settings(&self) -> &CaldirSettings {
        &self.settings
    }

    /// Persist the current settings back to the config file.
    pub fn save_config(&self) -> CalDirResult<()> {
        self.settings.save_config()
    }

    pub fn data_path(&self) -> PathBuf {
        self.settings.calendar_dir()
    }

    pub fn providers(&self) -> &[Provider] {
        self.settings.providers()
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.settings.provider_names()
    }

    pub fn provider(&self, name: &str) -> CalDirResult<Provider> {
        self.settings.provider(name)
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
        let Some(name) = self.settings.default_calendar() else {
            return Ok(None);
        };
        Ok(self.calendars()?.into_iter().find(|c| c.slug == name))
    }

    /// Set the default calendar if one isn't already configured.
    /// Returns true if the default was set.
    pub fn set_default_calendar_if_unset(&mut self, slug: &str) -> bool {
        self.settings.set_default_calendar_if_unset(slug)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use crate::caldir_settings::CaldirSettings;
    use std::ops::{Deref, DerefMut};
    use tempfile::TempDir;

    pub struct TestCaldir {
        _tmp: TempDir,
        caldir: Caldir,
    }

    impl TestCaldir {
        pub fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let calendar_dir = tmp.path().join("caldir");
            let config_path = tmp.path().join("config.toml");
            std::fs::create_dir_all(&calendar_dir).unwrap();
            let settings = CaldirSettings::from_config(
                &config_path,
                CaldirConfig {
                    calendar_dir,
                    ..CaldirConfig::new()
                },
            );
            let caldir = Caldir::new(settings);

            Self { _tmp: tmp, caldir }
        }
    }

    impl Deref for TestCaldir {
        type Target = Caldir;

        fn deref(&self) -> &Self::Target {
            &self.caldir
        }
    }

    impl DerefMut for TestCaldir {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.caldir
        }
    }

    /// Build a fresh Caldir rooted at a tempdir, with no providers.
    /// The wrapper owns the tempdir for the test's lifetime.
    pub fn mock_caldir() -> TestCaldir {
        TestCaldir::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use crate::error::CalDirError;
    use test_support::mock_caldir;

    #[test]
    fn calendars_returns_error_for_invalid_calendar_config() {
        let caldir = mock_caldir();
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
        let settings = CaldirSettings::from_config(
            tmp.path().join("config.toml"),
            CaldirConfig {
                calendar_dir: tmp.path().join("missing"),
                ..CaldirConfig::new()
            },
        );
        let caldir = Caldir::new(settings);

        assert!(caldir.calendars().unwrap().is_empty());
    }

    #[test]
    fn save_config_writes_to_the_configured_path() {
        let mut caldir = mock_caldir();
        let config_path = caldir.settings().config_path().to_path_buf();

        assert!(caldir.set_default_calendar_if_unset("work"));
        caldir.save_config().unwrap();

        let contents = std::fs::read_to_string(config_path).unwrap();
        assert!(contents.contains("default_calendar = \"work\""));
    }
}
