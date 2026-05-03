//! Caldir root directory management.

use std::path::{Path, PathBuf};

use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;
use config::{Config, File};

/// Filesystem and process environment used to construct a [`Caldir`] in
/// production. Resolves platform config paths and provider search dirs.
///
/// Tests don't need this — build a `Caldir` directly with [`Caldir::new`].
#[derive(Clone, Debug)]
pub struct CaldirEnvironment {
    config_path: PathBuf,
    provider_search_dirs: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
    config_path: Option<PathBuf>,
    providers: Vec<Provider>,
}

impl CaldirEnvironment {
    pub fn from_process() -> CalDirResult<Self> {
        Ok(Self {
            config_path: CaldirConfig::config_path()?,
            provider_search_dirs: Self::provider_search_dirs_from_process(),
        })
    }

    /// Bare-bones environment anchored at `config_path` with no provider
    /// search dirs. Intended for tests of the environment itself; chain
    /// [`with_provider_search_dirs`](Self::with_provider_search_dirs) to add
    /// any providers the test needs.
    pub fn at(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            provider_search_dirs: Vec::new(),
        }
    }

    pub fn with_provider_search_dirs<I, P>(mut self, dirs: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.provider_search_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn load(&self) -> CalDirResult<Caldir> {
        let config = self.load_config()?;
        let providers_data_dir = self.providers_data_dir_for(&config);
        let providers =
            Provider::discover_installed(&providers_data_dir, self.provider_search_dirs.iter());

        Ok(Caldir::new(config, providers).with_config_path(self.config_path.clone()))
    }

    pub fn providers_data_dir_for(&self, config: &CaldirConfig) -> PathBuf {
        config
            .providers_data_dir
            .as_ref()
            .map(|path| expand_tilde(path))
            .unwrap_or_else(|| default_providers_data_dir(&self.config_path))
    }

    fn load_config(&self) -> CalDirResult<CaldirConfig> {
        if !self.config_path.exists() {
            CaldirConfig::create_default_config(&self.config_path)?;
        }

        Config::builder()
            .add_source(File::from(self.config_path.clone()).required(false))
            .build()
            .map_err(|e| CalDirError::Config(e.to_string()))?
            .try_deserialize()
            .map_err(|e| CalDirError::Config(e.to_string()))
    }

    /// Returns directories from `CALDIR_PROVIDER_PATH` followed by `PATH`.
    fn provider_search_dirs_from_process() -> Vec<PathBuf> {
        let provider_path = std::env::var_os("CALDIR_PROVIDER_PATH");
        let system_path = std::env::var_os("PATH");
        provider_path
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .chain(
                system_path
                    .into_iter()
                    .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()),
            )
            .collect()
    }
}

impl Caldir {
    /// Load from the current process environment.
    ///
    /// Tests should use [`Caldir::new`] with an explicit config and provider
    /// list instead.
    pub fn load() -> CalDirResult<Self> {
        CaldirEnvironment::from_process()?.load()
    }

    /// Construct a Caldir directly from a config and provider list.
    ///
    /// `save_config` is a no-op until a path is set with
    /// [`with_config_path`](Self::with_config_path).
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
    pub fn set_default_calendar_if_unset(&mut self, slug: &str) -> bool {
        if self.config.default_calendar.is_some() {
            return false;
        }
        self.config.default_calendar = Some(slug.to_string());
        true
    }
}

fn default_providers_data_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("providers")
}

fn expand_tilde(path: &Path) -> PathBuf {
    let path = path.to_string_lossy();
    PathBuf::from(shellexpand::tilde(path.as_ref()).into_owned())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use tempfile::TempDir;

    /// Build a fresh Caldir rooted at a tempdir, with no providers and no
    /// config save path. The TempDir is returned alongside so it stays alive
    /// for the test's lifetime — drop it and the calendar dir disappears.
    ///
    /// Chain [`Caldir::with_config_path`] on the returned Caldir if the test
    /// needs `save_config` to actually write somewhere.
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
    fn environment_loads_provider_data_next_to_config_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("profile/config.toml");
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("caldir-provider-google"), "").unwrap();

        let environment =
            CaldirEnvironment::at(&config_path).with_provider_search_dirs([bin_dir.clone()]);
        let caldir = environment.load().unwrap();
        let provider = caldir.provider("google").unwrap();

        assert_eq!(
            environment.providers_data_dir_for(caldir.config()),
            tmp.path().join("profile/providers")
        );
        assert_eq!(
            provider.provider_dir(),
            tmp.path().join("profile/providers/google")
        );
    }

    #[test]
    fn mock_caldir_builds_a_minimal_instance_for_tests() {
        let (tmp, mut caldir) = mock_caldir();

        assert_eq!(caldir.data_path(), tmp.path().join("caldir"));
        assert!(caldir.providers().is_empty());
        assert!(caldir.config_path().is_none());
        assert!(caldir.set_default_calendar_if_unset("personal"));
        // No path set -> save_config is a no-op.
        caldir.save_config().unwrap();
    }

    #[test]
    fn explicit_provider_data_dir_overrides_config_path_default() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("profile/config.toml");
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("caldir-provider-google"), "").unwrap();
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        CaldirConfig {
            calendar_dir: tmp.path().join("calendars"),
            providers_data_dir: Some(tmp.path().join("provider-data")),
            ..CaldirConfig::default()
        }
        .save_to(&config_path)
        .unwrap();

        let environment =
            CaldirEnvironment::at(&config_path).with_provider_search_dirs([bin_dir.clone()]);
        let caldir = environment.load().unwrap();
        let provider = caldir.provider("google").unwrap();

        assert_eq!(
            environment.providers_data_dir_for(caldir.config()),
            tmp.path().join("provider-data")
        );
        assert_eq!(
            provider.provider_dir(),
            tmp.path().join("provider-data/google")
        );
    }

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
}
