//! Resolved runtime settings for a [`Caldir`](crate::caldir::Caldir).
//!
//! [`CaldirConfig`](crate::caldir_config::CaldirConfig) is the TOML file schema.
//! `CaldirSettings` is what the running process uses after defaults and paths
//! have been resolved.

use std::path::{Path, PathBuf};

use crate::caldir_config::{CaldirConfig, TimeFormat};
use crate::error::{CalDirError, CalDirResult};
use crate::event::Reminder;
use crate::utils::expand_tilde;

#[derive(Clone, Debug)]
pub struct CaldirSettings {
    config_path: PathBuf,
    config: CaldirConfig,
    provider_search_dirs: Vec<PathBuf>,
}

impl CaldirSettings {
    pub fn load() -> CalDirResult<Self> {
        Self::load_from(
            CaldirConfig::config_path()?,
            provider_search_dirs_from_process(),
        )
    }

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
        Ok(Self::from_config(config_path, config, provider_search_dirs))
    }

    pub fn from_config<I, P>(
        config_path: impl Into<PathBuf>,
        config: CaldirConfig,
        provider_search_dirs: I,
    ) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self {
            config_path: config_path.into(),
            config,
            provider_search_dirs: provider_search_dirs.into_iter().map(Into::into).collect(),
        }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn calendar_dir(&self) -> PathBuf {
        expand_tilde(&self.config.calendar_dir)
    }

    pub fn providers_data_dir(&self) -> PathBuf {
        self.config
            .providers_data_dir
            .as_ref()
            .map(|path| expand_tilde(path))
            .unwrap_or_else(|| default_providers_data_dir(&self.config_path))
    }

    pub fn provider_search_dirs(&self) -> &[PathBuf] {
        &self.provider_search_dirs
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

pub fn default_providers_data_dir(config_path: &Path) -> PathBuf {
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
    use crate::remote::provider::Provider;

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

    fn caldir_from_settings(settings: CaldirSettings) -> Caldir {
        let providers = Provider::discover_installed(
            settings.providers_data_dir(),
            settings.provider_search_dirs().iter(),
        );
        Caldir::new(settings, providers)
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
        let settings = CaldirSettings::load_from(&config_path, [&bin_dir]).unwrap();
        let caldir = caldir_from_settings(settings);

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

        let settings = CaldirSettings::load_from(&config_path, [&bin_dir]).unwrap();
        let caldir = caldir_from_settings(settings);

        assert_eq!(
            caldir.provider("google").unwrap().provider_dir(),
            custom_data_path.join("google")
        );
    }
}
