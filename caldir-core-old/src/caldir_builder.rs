use std::path::{Path, PathBuf};

use crate::{caldir_config::CaldirConfig, remote::provider::Provider};

#[derive(Default)]
pub struct CaldirBuilder {
    config_path: Option<PathBuf>,
    config: Option<CaldirConfig>,
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

    // pub fn build(self) -> CalDirResult<Caldir> {
    //     let config_path = match self.config_path {
    //         Some(path) => path,
    //         None => CaldirConfig::config_path()?,
    //     };
    //     let config = match self.config {
    //         Some(config) => config,
    //         None => CaldirConfig::load_from(&config_path)?,
    //     };
    //
    //     let dir = expand_tilde(&config.calendar_dir);
    //
    //     let providers_dir = config
    //         .providers_data_dir
    //         .as_ref()
    //         .map(|path| expand_tilde(path))
    //         .unwrap_or_else(|| default_providers_dir(&config_path));
    //
    //     let providers = match self.providers {
    //         Some(providers) => providers,
    //         None => Provider::discover_installed(&providers_dir, Self::default_bin_dirs()),
    //     };
    //
    //     Ok(Caldir::from_resolved(config_path, config, dir, providers))
    // }

    // $PATH value
    pub fn default_bin_dirs() -> Vec<PathBuf> {
        bin_dirs_from_env("PATH")
    }
}

fn bin_dirs_from_env(name: &str) -> Vec<PathBuf> {
    std::env::var_os(name)
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .collect()
}

fn default_providers_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("providers")
}
