mod config;
mod error;

use crate::{
    Calendar, CalendarConfig, Connection, Provider, ProviderRegistry, ProviderSlug, Remote,
};
use std::path::{Path, PathBuf};

pub use config::TimeFormat;
pub use config::{CaldirConfig, CaldirConfigError};
pub use error::CaldirError;

pub struct Caldir {
    config: CaldirConfig,
    config_path: Option<PathBuf>,
    providers: ProviderRegistry,
}

impl Caldir {
    #[cfg(test)]
    pub(crate) fn new(config: CaldirConfig, providers: ProviderRegistry) -> Self {
        Caldir {
            config,
            config_path: None,
            providers,
        }
    }

    /// Load from the system config path and scan `PATH` for providers.
    pub fn load() -> Result<Self, CaldirError> {
        Self::load_from(CaldirConfig::default_system_config_path()?)
    }

    /// Like `load`, but reads the config from an explicit path.
    pub fn load_from(config_path: impl Into<PathBuf>) -> Result<Self, CaldirError> {
        let config_path = config_path.into();
        let config = CaldirConfig::load_or_default(&config_path)?;
        let providers = ProviderRegistry::from_system_path();

        Ok(Self {
            config,
            config_path: Some(config_path),
            providers,
        })
    }

    /// Register bundled providers from `dir`, overriding PATH ones on conflict.
    pub fn with_bundled_providers(mut self, dir: impl AsRef<Path>) -> Self {
        self.providers.add_from_dir(dir);
        self
    }

    /// Replace the provider registry, e.g. after rescanning `PATH`.
    pub fn set_providers(&mut self, providers: ProviderRegistry) {
        self.providers = providers;
    }

    /// Override the RPC timeout for all providers.
    pub fn set_provider_timeout(&mut self, timeout: std::time::Duration) {
        self.providers.set_timeout(timeout);
    }

    pub fn data_dir(&self) -> PathBuf {
        self.config.data_dir()
    }

    pub fn default_calendar(&self) -> Result<Calendar, CaldirError> {
        let slug = self
            .config
            .default_calendar_slug()
            .ok_or(CaldirError::NoDefaultCalendar)?;

        Ok(Calendar::load(&self.data_dir().join(slug))?)
    }

    pub fn create_calendar(
        &self,
        desired_slug: &str,
        config: Option<CalendarConfig>,
    ) -> Result<Calendar, CaldirError> {
        let unique_slug = self.unique_slug_for(desired_slug);
        let calendar_path = self.data_dir().join(unique_slug);

        Ok(Calendar::create(&calendar_path, config)?)
    }

    pub fn calendars(&self) -> Vec<Result<Calendar, CaldirError>> {
        let mut calendars = Vec::new();

        if let Ok(entries) = std::fs::read_dir(self.data_dir()) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                    calendars.push(Calendar::load(&entry.path()).map_err(CaldirError::from));
                }
            }
        }

        calendars
    }

    pub fn calendar(&self, slug: &str) -> Result<Calendar, CaldirError> {
        Calendar::load(&self.data_dir().join(slug)).map_err(CaldirError::from)
    }

    pub fn connections(&self) -> Vec<Result<Connection, CaldirError>> {
        let mut connections = Vec::new();

        for calendar in self.calendars() {
            let calendar = match calendar {
                Ok(calendar) => calendar,
                Err(err) => {
                    connections.push(Err(err));
                    continue;
                }
            };

            let Some(remote_config) = calendar.remote_config().cloned() else {
                continue;
            };

            let connection = self
                .provider(remote_config.provider_slug())
                .map(|provider| {
                    Connection::new(
                        calendar,
                        Remote::new(provider.clone(), remote_config.params().clone()),
                    )
                });

            connections.push(connection);
        }

        connections
    }

    pub fn providers(&self) -> &ProviderRegistry {
        &self.providers
    }

    pub fn provider(&self, provider_slug: &ProviderSlug) -> Result<&Provider, CaldirError> {
        self.providers
            .get(provider_slug)
            .map_err(CaldirError::Provider)
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    /// Persist `new_config` to disk and adopt it as the in-memory config.
    /// Either both sides commit or neither — on write failure the in-memory
    /// config is left untouched.
    pub fn save_config(&mut self, new_config: CaldirConfig) -> Result<(), CaldirError> {
        if let Some(path) = &self.config_path {
            new_config.write(path)?;
        }

        self.config = new_config;

        Ok(())
    }

    /// Re-read the config from disk, keeping the provider registry. On failure
    /// the in-memory config is left untouched. No-op without a config path.
    pub fn reload_config(&mut self) -> Result<(), CaldirError> {
        if let Some(path) = &self.config_path {
            self.config = CaldirConfig::load_or_default(path)?;
        }

        Ok(())
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    fn unique_slug_for(&self, desired_slug: &str) -> String {
        let calendar_dir = self.config.data_dir();

        if !calendar_dir.join(desired_slug).exists() {
            return desired_slug.to_string();
        }

        let mut suffix = 2;

        loop {
            let candidate = format!("{desired_slug}-{suffix}");
            if !calendar_dir.join(&candidate).exists() {
                return candidate;
            }
            suffix += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderError;
    use crate::test_utils::{
        test_caldir, test_caldir_config, test_calendar_config, test_provider, test_remote_config,
    };

    #[test]
    fn create_calendar_creates_directory_with_desired_slug() {
        let (_tmp, caldir) = test_caldir();

        let calendar = caldir.create_calendar("work", None).unwrap();

        assert_eq!(calendar.path(), caldir.data_dir().join("work"));
        assert_eq!(calendar.slug().unwrap(), "work");
        assert!(calendar.path().is_dir());
    }

    #[test]
    fn create_appends_suffix_on_slug_collision() {
        let (_tmp, caldir) = test_caldir();

        let calendar_1 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_1.slug().unwrap(), "work");

        let calendar_2 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_2.slug().unwrap(), "work-2");

        let calendar_3 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_3.slug().unwrap(), "work-3");
    }

    #[test]
    fn calendars_returns_empty_if_no_calendars() {
        let (_tmp, caldir) = test_caldir();

        assert!(caldir.calendars().is_empty());
    }

    #[test]
    fn calendars_returns_each_calendar_subdirectory() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("personal", None).unwrap();
        caldir.create_calendar("work", None).unwrap();

        let calendars = caldir.calendars();

        let mut slugs: Vec<String> = calendars
            .iter()
            .map(|c| c.as_ref().unwrap().slug().unwrap().to_string())
            .collect();

        slugs.sort();

        assert_eq!(slugs, vec!["personal", "work"]);
    }

    #[test]
    fn calendars_ignores_hidden_directories() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("work", None).unwrap();
        std::fs::create_dir_all(caldir.data_dir().join(".hidden")).unwrap();
        std::fs::create_dir_all(caldir.data_dir().join(".git")).unwrap();

        let calendars = caldir.calendars();

        let slugs: Vec<String> = calendars
            .iter()
            .map(|c| c.as_ref().unwrap().slug().unwrap().to_string())
            .collect();

        assert_eq!(slugs, vec!["work"]);
    }

    #[test]
    fn connections_is_empty_when_no_calendars_exist() {
        let (_tmp, caldir) = test_caldir();

        assert!(caldir.connections().is_empty());
    }

    #[test]
    fn connections_skips_calendars_without_remote() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("local-only", None).unwrap();

        assert!(caldir.connections().is_empty());
    }

    #[test]
    fn connections_returns_calendar_with_remote() {
        let (_tmp_bin, provider) = test_provider("hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(provider);

        let (_tmp, config) = test_caldir_config();
        let caldir = Caldir::new(config, registry);

        let remote_config = test_remote_config("hooli");
        let mut config = test_calendar_config();
        config.set_remote(remote_config);

        caldir.create_calendar("work", Some(config)).unwrap();
        caldir.create_calendar("local-only", None).unwrap();

        let connections = caldir.connections();

        assert_eq!(connections.len(), 1);
        let connection = connections[0].as_ref().unwrap();
        assert_eq!(connection.local().slug().unwrap(), "work");
    }

    #[test]
    fn connections_returns_err_for_calendar_with_missing_provider() {
        let (_tmp, caldir) = test_caldir();

        let remote_config = test_remote_config("hooli");
        let mut config = test_calendar_config();
        config.set_remote(remote_config);

        caldir.create_calendar("work", Some(config)).unwrap();

        let connections = caldir.connections();

        assert_eq!(connections.len(), 1);
        assert!(matches!(
            connections[0],
            Err(CaldirError::Provider(ProviderError::ProviderNotFound(_)))
        ));
    }

    #[test]
    fn connections_returns_ok_and_err_independently_per_calendar() {
        let (_tmp_bin, provider) = test_provider("hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(provider);

        let (_tmp, config) = test_caldir_config();
        let caldir = Caldir::new(config, registry);

        let mut work_config = test_calendar_config();
        work_config.set_remote(test_remote_config("hooli"));
        caldir.create_calendar("work", Some(work_config)).unwrap();

        let mut other_config = test_calendar_config();
        other_config.set_remote(test_remote_config("aviato"));
        caldir.create_calendar("other", Some(other_config)).unwrap();

        let connected = caldir.connections();

        assert_eq!(connected.len(), 2);
        assert_eq!(connected.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(connected.iter().filter(|r| r.is_err()).count(), 1);
    }

    #[test]
    fn provider_returns_provider_when_present_in_registry() {
        let (_tmp_bin, provider) = test_provider("hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(provider);

        let (_tmp, config) = test_caldir_config();
        let caldir = Caldir::new(config, registry);

        assert!(caldir.provider(&ProviderSlug::from("hooli")).is_ok());
    }

    #[test]
    fn provider_errors_when_not_present_in_registry() {
        let (_tmp, caldir) = test_caldir();

        let result = caldir.provider(&ProviderSlug::from("hooli"));

        assert!(matches!(
            result,
            Err(CaldirError::Provider(ProviderError::ProviderNotFound(_)))
        ));
    }

    #[test]
    fn default_calendar_returns_calendar_matching_configured_slug() {
        let (_tmp, mut config) = test_caldir_config();
        config = CaldirConfig::new(
            config.data_dir(),
            TimeFormat::default(),
            Some("personal".to_string()),
            None,
        );
        let caldir = Caldir::new(config, ProviderRegistry::new());

        caldir.create_calendar("personal", None).unwrap();
        caldir.create_calendar("work", None).unwrap();

        let calendar = caldir.default_calendar().unwrap();

        assert_eq!(calendar.slug().unwrap(), "personal");
    }

    #[test]
    fn default_calendar_errors_when_no_default_slug_configured() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("personal", None).unwrap();

        assert!(matches!(
            caldir.default_calendar(),
            Err(CaldirError::NoDefaultCalendar)
        ));
    }

    fn write_config(path: &Path, data_dir: &str) {
        std::fs::write(path, format!("calendar_dir = \"{data_dir}\"\n")).unwrap();
    }

    #[test]
    fn load_from_reads_config_at_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(&path, "/tmp/from-disk");

        let caldir = Caldir::load_from(&path).unwrap();

        assert_eq!(caldir.data_dir(), PathBuf::from("/tmp/from-disk"));
    }

    #[test]
    fn load_from_uses_defaults_when_file_is_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("missing.toml");

        let caldir = Caldir::load_from(&path).unwrap();

        assert_eq!(caldir.config(), &CaldirConfig::default());
    }

    #[test]
    fn load_from_errors_on_malformed_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "not a valid toml").unwrap();

        assert!(matches!(
            Caldir::load_from(&path),
            Err(CaldirError::Config(_))
        ));
    }

    #[test]
    fn save_config_writes_to_the_loaded_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        let mut caldir = Caldir::load_from(&path).unwrap();

        let mut config = caldir.config().clone();
        config.set_data_dir(PathBuf::from("/tmp/saved"));
        caldir.save_config(config.clone()).unwrap();

        assert_eq!(CaldirConfig::load_or_default(&path).unwrap(), config);
    }

    #[test]
    fn reload_config_picks_up_changes_on_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(&path, "/tmp/before");
        let mut caldir = Caldir::load_from(&path).unwrap();

        write_config(&path, "/tmp/after");
        caldir.reload_config().unwrap();

        assert_eq!(caldir.data_dir(), PathBuf::from("/tmp/after"));
    }

    #[test]
    fn reload_config_keeps_previous_config_on_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(&path, "/tmp/before");
        let mut caldir = Caldir::load_from(&path).unwrap();

        std::fs::write(&path, "not a valid toml").unwrap();

        assert!(matches!(
            caldir.reload_config(),
            Err(CaldirError::Config(_))
        ));
        assert_eq!(caldir.data_dir(), PathBuf::from("/tmp/before"));
    }

    #[test]
    fn reload_config_keeps_providers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        let mut caldir = Caldir::load_from(&path).unwrap();

        let (_tmp_bin, provider) = test_provider("hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(provider);
        caldir.set_providers(registry);

        write_config(&path, "/tmp/after");
        caldir.reload_config().unwrap();

        assert!(caldir.provider(&ProviderSlug::from("hooli")).is_ok());
    }

    #[test]
    fn reload_config_is_noop_without_config_path() {
        let (_tmp, mut caldir) = test_caldir();
        let before = caldir.config().clone();

        caldir.reload_config().unwrap();

        assert_eq!(caldir.config(), &before);
    }

    #[test]
    fn set_providers_replaces_registry() {
        let (_tmp_bin, hooli) = test_provider("hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(hooli);
        let (_tmp, config) = test_caldir_config();
        let mut caldir = Caldir::new(config, registry);

        let (_tmp_bin, aviato) = test_provider("aviato");
        let mut registry = ProviderRegistry::new();
        registry.add(aviato);
        caldir.set_providers(registry);

        assert!(caldir.provider(&ProviderSlug::from("hooli")).is_err());
        assert!(caldir.provider(&ProviderSlug::from("aviato")).is_ok());
    }

    #[test]
    fn default_calendar_errors_when_calendar_does_not_exist() {
        let (_tmp, mut config) = test_caldir_config();
        config = CaldirConfig::new(
            config.data_dir(),
            TimeFormat::default(),
            Some("missing".to_string()),
            None,
        );
        let caldir = Caldir::new(config, ProviderRegistry::new());

        assert!(matches!(
            caldir.default_calendar(),
            Err(CaldirError::Calendar(_))
        ));
    }
}
