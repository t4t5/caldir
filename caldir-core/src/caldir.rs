//! Caldir root directory management.

use std::path::{Path, PathBuf};

use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;
use config::{Config, File};

#[derive(Clone)]
pub struct CaldirOptions {
    pub config: CaldirConfig,
    pub config_store: CaldirConfigStore,
    pub providers: Vec<Provider>,
}

#[derive(Clone, Debug)]
pub enum CaldirConfigStore {
    File(PathBuf),
    Memory,
}

/// Filesystem and process environment used to construct a [`Caldir`].
///
/// This is the boundary where platform config paths, provider search paths,
/// and config persistence live. `Caldir` itself is just the runtime context.
#[derive(Clone, Debug)]
pub struct CaldirEnvironment {
    config_path: PathBuf,
    provider_search_dirs: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
    config_store: CaldirConfigStore,
    providers: Vec<Provider>,
}

impl CaldirConfigStore {
    pub fn path(&self) -> Option<&Path> {
        match self {
            CaldirConfigStore::File(path) => Some(path.as_path()),
            CaldirConfigStore::Memory => None,
        }
    }

    pub fn save(&self, config: &CaldirConfig) -> CalDirResult<()> {
        match self {
            CaldirConfigStore::File(path) => config.save_to(path),
            CaldirConfigStore::Memory => Ok(()),
        }
    }
}

impl CaldirEnvironment {
    pub fn from_process() -> CalDirResult<Self> {
        Ok(Self {
            config_path: CaldirConfig::config_path()?,
            provider_search_dirs: Self::provider_search_dirs_from_process(),
        })
    }

    /// Bare-bones environment anchored at `config_path` with no provider
    /// search dirs. Intended for tests; chain
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
        Ok(Caldir::new(self.load_options()?))
    }

    pub fn load_options(&self) -> CalDirResult<CaldirOptions> {
        let config = self.load_config()?;
        let providers_data_dir = self.providers_data_dir_for(&config);
        let providers =
            Provider::discover_installed(&providers_data_dir, self.provider_search_dirs.iter());

        Ok(CaldirOptions {
            config,
            config_store: CaldirConfigStore::File(self.config_path.clone()),
            providers,
        })
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
    /// Tests that need deterministic paths/providers should use
    /// [`Caldir::new`] with explicit [`CaldirOptions`] instead.
    pub fn load() -> CalDirResult<Self> {
        CaldirEnvironment::from_process()?.load()
    }

    pub fn new(options: CaldirOptions) -> Self {
        Caldir {
            config: options.config,
            config_store: options.config_store,
            providers: options.providers,
        }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn config_path(&self) -> Option<&Path> {
        self.config_store.path()
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config_store.save(&self.config)
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
mod tests {
    use super::*;

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
    fn caldir_can_be_constructed_from_resolved_options_without_environment() {
        let tmp = tempfile::tempdir().unwrap();
        let mut caldir = Caldir::new(CaldirOptions {
            config: CaldirConfig {
                calendar_dir: tmp.path().join("caldir"),
                ..CaldirConfig::default()
            },
            config_store: CaldirConfigStore::Memory,
            providers: Vec::new(),
        });

        assert_eq!(caldir.data_path(), tmp.path().join("caldir"));
        assert!(caldir.providers().is_empty());
        assert!(caldir.config_path().is_none());
        assert!(caldir.set_default_calendar_if_unset("personal"));
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
    fn file_backed_caldir_saves_config_to_its_store() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        let mut caldir = Caldir::new(CaldirOptions {
            config: CaldirConfig {
                calendar_dir: tmp.path().join("calendars"),
                ..CaldirConfig::default()
            },
            config_store: CaldirConfigStore::File(config_path.clone()),
            providers: Vec::new(),
        });

        assert_eq!(caldir.config_path(), Some(config_path.as_path()));
        assert!(caldir.set_default_calendar_if_unset("work"));
        caldir.save_config().unwrap();

        let contents = std::fs::read_to_string(config_path).unwrap();
        assert!(contents.contains("default_calendar = \"work\""));
    }
}
