use std::path::{Path, PathBuf};

use crate::{
    caldir::Caldir, caldir_config::CaldirConfig, error::CalDirResult, remote::provider::Provider,
    utils::expand_tilde,
};

#[derive(Default)]
pub struct CaldirBuilder {
    config_path: Option<PathBuf>,
    config: Option<CaldirConfig>,
    provider_search_dirs: Option<Vec<PathBuf>>,
    providers: Option<Vec<Provider>>,
}

impl CaldirBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    pub fn config(mut self, config: CaldirConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set provider binary search directories. This overrides the default `PATH`
    /// lookup, so tests can use an empty list and GUI apps can prepend bundled
    /// provider directories.
    pub fn provider_search_dirs<I, P>(mut self, dirs: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.provider_search_dirs = Some(dirs.into_iter().map(Into::into).collect());
        self
    }

    /// Provide an already-resolved provider snapshot directly.
    pub fn providers(mut self, providers: Vec<Provider>) -> Self {
        self.providers = Some(providers);
        self
    }

    /// Disable provider discovery and build with an empty provider snapshot.
    pub fn without_providers(mut self) -> Self {
        self.providers = Some(Vec::new());
        self
    }

    pub fn build(self) -> CalDirResult<Caldir> {
        let config_path = match self.config_path {
            Some(path) => path,
            None => CaldirConfig::config_path()?,
        };
        let config = match self.config {
            Some(config) => config,
            None => CaldirConfig::load_from(&config_path)?,
        };

        let dir = expand_tilde(&config.calendar_dir);
        let providers_dir = config
            .providers_data_dir
            .as_ref()
            .map(|path| expand_tilde(path))
            .unwrap_or_else(|| default_providers_dir(&config_path));
        let providers = match self.providers {
            Some(providers) => providers,
            None => {
                let provider_search_dirs = self
                    .provider_search_dirs
                    .unwrap_or_else(Self::default_provider_search_dirs);
                Provider::discover_installed(&providers_dir, provider_search_dirs)
            }
        };

        Ok(Caldir::from_resolved(config_path, config, dir, providers))
    }

    /// Returns the default provider binary search dirs from `PATH`.
    pub fn default_provider_search_dirs() -> Vec<PathBuf> {
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

fn default_providers_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("providers")
}
