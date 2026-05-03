//! Caldir root directory management.

use std::path::{Path, PathBuf};

use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;
use config::{Config, File};

#[derive(Clone)]
pub struct Caldir {
    config: CaldirConfig,
    installed_providers: Vec<Provider>,
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

        let providers_data_dir = Self::effective_providers_data_dir(&config, &config_path);
        let installed_providers = Provider::discover_installed(&providers_data_dir);

        Ok(Caldir {
            config,
            installed_providers,
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
        let config = CaldirConfig {
            calendar_dir: data_path,
            ..CaldirConfig::default()
        };
        let providers_data_dir = Self::effective_providers_data_dir(&config, &config_path);
        let installed_providers = Provider::discover_installed(&providers_data_dir);

        Caldir {
            config,
            installed_providers,
            config_path,
        }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn data_path(&self) -> PathBuf {
        Self::expand_tilde(&self.config.calendar_dir)
    }

    /// Returns the calendar directory path in display-friendly form,
    /// keeping `~` instead of expanding to the full home directory.
    pub fn display_path(&self) -> PathBuf {
        self.config.calendar_dir.clone()
    }

    /// Directory containing provider-private data directories.
    ///
    /// For the default config this is `~/.config/caldir/providers`. For tests
    /// loaded from an explicit config path, provider data stays next to that
    /// config unless `providers_data_dir` is set explicitly.
    pub fn providers_data_dir(&self) -> PathBuf {
        Self::effective_providers_data_dir(&self.config, &self.config_path)
    }

    fn effective_providers_data_dir(config: &CaldirConfig, config_path: &Path) -> PathBuf {
        config
            .providers_data_dir
            .as_ref()
            .map(|path| Self::expand_tilde(path))
            .unwrap_or_else(|| Self::default_providers_data_dir(config_path))
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

    pub fn installed_providers(&self) -> &[Provider] {
        &self.installed_providers
    }

    pub fn installed_provider_names(&self) -> Vec<String> {
        self.installed_providers
            .iter()
            .map(|provider| provider.name().to_string())
            .collect()
    }

    pub fn provider(&self, name: &str) -> CalDirResult<Provider> {
        self.installed_providers
            .iter()
            .find(|provider| provider.name() == name)
            .cloned()
            .ok_or_else(|| CalDirError::ProviderNotInstalled(name.to_string()))
    }

    pub fn provider_dir(&self, name: &str) -> PathBuf {
        self.providers_data_dir().join(name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_data_stays_next_to_loaded_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("profile/config.toml");

        let caldir = Caldir::load(&config_path).unwrap();

        assert_eq!(
            caldir.providers_data_dir(),
            tmp.path().join("profile/providers")
        );
        assert_eq!(
            caldir.provider_dir("google"),
            tmp.path().join("profile/providers/google")
        );
    }

    #[test]
    fn with_data_path_keeps_provider_data_in_data_path() {
        let tmp = tempfile::tempdir().unwrap();
        let caldir = Caldir::with_data_path(tmp.path().join("caldir"));

        assert_eq!(
            caldir.provider_dir("icloud"),
            tmp.path().join("caldir/providers/icloud")
        );
    }

    #[test]
    fn explicit_provider_data_dir_overrides_config_path_default() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("profile/config.toml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        CaldirConfig {
            calendar_dir: tmp.path().join("calendars"),
            providers_data_dir: Some(tmp.path().join("provider-data")),
            ..CaldirConfig::default()
        }
        .save_to(&config_path)
        .unwrap();

        let caldir = Caldir::load(&config_path).unwrap();

        assert_eq!(
            caldir.providers_data_dir(),
            tmp.path().join("provider-data")
        );
        assert_eq!(
            caldir.provider_dir("google"),
            tmp.path().join("provider-data/google")
        );
    }
}
