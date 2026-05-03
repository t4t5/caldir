//! Filesystem and process environment used to construct a [`Caldir`] in
//! production. Resolves platform config paths and provider search dirs.
//!
//! Tests don't need this — build a `Caldir` directly with [`Caldir::new`].

use std::path::{Path, PathBuf};

use config::{Config, File};

use crate::caldir::Caldir;
use crate::caldir_config::CaldirConfig;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;
use crate::utils::expand_tilde;

#[derive(Clone, Debug)]
pub struct CaldirEnvironment {
    config_path: PathBuf,
    provider_search_dirs: Vec<PathBuf>,
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

    #[test]
    fn provider_data_is_stored_next_to_config_by_default() {
        let home = tempfile::tempdir().unwrap();

        // provider binary:
        let bin_dir = home.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("caldir-provider-google"), "").unwrap();

        // custom config path:
        let random_path = &home.path().join("random_path");
        let config_path = random_path.join("config.toml");

        let caldir = CaldirEnvironment::at(&config_path)
            .with_provider_search_dirs([bin_dir])
            .load()
            .unwrap();

        assert_eq!(
            caldir.provider("google").unwrap().provider_dir(),
            random_path.join("providers/google")
        );
    }

    #[test]
    fn provider_data_can_be_stored_elsewhere() {
        let home = tempfile::tempdir().unwrap();

        // provider binary:
        let bin_dir = &home.path().join("bin");
        std::fs::create_dir_all(bin_dir).unwrap();
        std::fs::write(bin_dir.join("caldir-provider-google"), "").unwrap();

        // custom config path:
        let random_path = &home.path().join("random_path");
        let config_path = random_path.join("config.toml");
        std::fs::create_dir_all(random_path).unwrap();

        // config explicitly stores provider data elsewhere:
        let custom_data_path = home.path().join("elsewhere");
        CaldirConfig {
            calendar_dir: home.path().join("calendars"),
            providers_data_dir: Some(custom_data_path.clone()),
            ..CaldirConfig::default()
        }
        .save_to(&config_path)
        .unwrap();

        let caldir = CaldirEnvironment::at(&config_path)
            .with_provider_search_dirs([bin_dir])
            .load()
            .unwrap();

        assert_eq!(
            caldir.provider("google").unwrap().provider_dir(),
            custom_data_path.join("google")
        );
    }
}
