//! Resolved runtime environment for a [`Caldir`](crate::caldir::Caldir).
//!
//! [`CaldirConfig`](crate::caldir_config::CaldirConfig) is the TOML file schema.
//! `CaldirEnvironment` is what the running process uses after defaults, paths,
//! and provider discovery have been resolved.

use std::path::{Path, PathBuf};

use crate::caldir_config::{CaldirConfig, TimeFormat};
use crate::error::{CalDirError, CalDirResult};
use crate::event::Reminder;
use crate::remote::provider::Provider;
use crate::utils::expand_tilde;

#[derive(Clone, Debug)]
pub struct CaldirEnvironment {
    config_path: PathBuf,
    config: CaldirConfig,
    calendar_dir: PathBuf,
    providers_data_dir: PathBuf,
    providers: Vec<Provider>,
}

impl CaldirEnvironment {
    pub fn load() -> CalDirResult<Self> {
        Self::load_from(CaldirConfig::config_path()?, Self::provider_search_dirs())
    }

    /// Load an environment from a config path and discover providers from the given
    /// search dirs. The search dirs are construction input only; the resulting
    /// environment stores the resolved provider snapshot.
    pub fn load_from<I, P>(
        config_path: impl Into<PathBuf>,
        provider_search_dirs: I,
    ) -> CalDirResult<Self>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let config_path = config_path.into();
        let config = CaldirConfig::load_from(&config_path)?;
        let provider_search_dirs: Vec<PathBuf> =
            provider_search_dirs.into_iter().map(Into::into).collect();
        Ok(Self::from_config(config_path, config).with_discovered_providers(provider_search_dirs))
    }

    pub fn from_config(config_path: impl Into<PathBuf>, config: CaldirConfig) -> Self {
        let config_path = config_path.into();
        let calendar_dir = expand_tilde(&config.calendar_dir);
        let providers_data_dir = config
            .providers_data_dir
            .as_ref()
            .map(|path| expand_tilde(path))
            .unwrap_or_else(|| default_providers_data_dir(&config_path));

        Self {
            config_path,
            config,
            calendar_dir,
            providers_data_dir,
            providers: Vec::new(),
        }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn calendar_dir(&self) -> PathBuf {
        self.calendar_dir.clone()
    }

    pub fn providers_data_dir(&self) -> PathBuf {
        self.providers_data_dir.clone()
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

    pub fn with_providers(mut self, providers: Vec<Provider>) -> Self {
        self.providers = providers;
        self
    }

    pub fn with_discovered_providers<I, P>(self, provider_search_dirs: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let providers =
            Provider::discover_installed(self.providers_data_dir(), provider_search_dirs);
        self.with_providers(providers)
    }

    pub fn default_calendar(&self) -> Option<&str> {
        self.config.default_calendar.as_deref()
    }

    pub fn time_format(&self) -> TimeFormat {
        self.config.time_format
    }

    /// Parse default_reminders strings into Reminder structs.
    pub fn parse_default_reminders(&self) -> CalDirResult<Option<Vec<Reminder>>> {
        let Some(ref strs) = self.config.default_reminders else {
            return Ok(None);
        };
        let reminders: Vec<Reminder> = strs
            .iter()
            .map(|s| Reminder::from_duration_str(s).map_err(CalDirError::Config))
            .collect::<CalDirResult<_>>()?;
        Ok(Some(reminders))
    }

    pub fn set_default_calendar_if_unset(&mut self, slug: &str) -> bool {
        if self.config.default_calendar.is_some() {
            return false;
        }
        self.config.default_calendar = Some(slug.to_string());
        true
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save_to(&self.config_path)
    }

    /// Returns the default provider binary search dirs from `PATH`.
    /// GUI clients can prepend bundled provider dirs to this list and pass the
    /// combined list to [`Self::load_from`].
    pub fn provider_search_dirs() -> Vec<PathBuf> {
        unique_paths(provider_search_dirs_from_env("PATH"))
    }
}

fn provider_search_dirs_from_env(name: &str) -> Vec<PathBuf> {
    std::env::var_os(name)
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .collect()
}

fn unique_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::new();
    for path in paths {
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    unique
}

fn default_providers_data_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("providers")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldir::Caldir;

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &std::path::Path) {}

    fn provider_binary_name(provider: &str) -> String {
        format!("caldir-provider-{provider}{}", std::env::consts::EXE_SUFFIX)
    }

    #[test]
    fn paths_are_resolved_when_environment_is_constructed() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");

        let environment = CaldirEnvironment::from_config(
            &config_path,
            CaldirConfig {
                calendar_dir: "~/calendars".into(),
                providers_data_dir: Some("~/provider-data".into()),
                ..CaldirConfig::new()
            },
        );

        assert_eq!(
            environment.config().calendar_dir,
            PathBuf::from("~/calendars")
        );
        assert_eq!(
            environment.config().providers_data_dir,
            Some(PathBuf::from("~/provider-data"))
        );
        assert_eq!(
            environment.calendar_dir(),
            crate::utils::expand_tilde(std::path::Path::new("~/calendars"))
        );
        assert_eq!(
            environment.providers_data_dir(),
            crate::utils::expand_tilde(std::path::Path::new("~/provider-data"))
        );
    }

    #[test]
    fn provider_data_is_stored_next_to_config_by_default() {
        let home = tempfile::tempdir().unwrap();

        let bin_dir = home.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let provider_binary = bin_dir.join(provider_binary_name("google"));
        std::fs::write(&provider_binary, "").unwrap();
        make_executable(&provider_binary);

        let random_path = home.path().join("random_path");
        let config_path = random_path.join("config.toml");

        let environment = CaldirEnvironment::load_from(&config_path, [&bin_dir]).unwrap();

        let caldir = Caldir::new(environment);

        assert_eq!(
            caldir.provider("google").unwrap().provider_dir(),
            random_path.join("providers/google")
        );
    }

    #[test]
    fn provider_data_can_be_stored_elsewhere() {
        let home = tempfile::tempdir().unwrap();

        let bin_dir = home.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let provider_binary = bin_dir.join(provider_binary_name("google"));
        std::fs::write(&provider_binary, "").unwrap();
        make_executable(&provider_binary);

        let random_path = home.path().join("random_path");
        let config_path = random_path.join("config.toml");

        let custom_data_path = home.path().join("elsewhere");
        CaldirConfig {
            calendar_dir: home.path().join("calendars"),
            providers_data_dir: Some(custom_data_path.clone()),
            ..CaldirConfig::new()
        }
        .save_to(&config_path)
        .unwrap();

        let environment = CaldirEnvironment::load_from(&config_path, [&bin_dir]).unwrap();
        let caldir = Caldir::new(environment);

        assert_eq!(
            caldir.provider("google").unwrap().provider_dir(),
            custom_data_path.join("google")
        );
    }
}
