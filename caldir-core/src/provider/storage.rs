//! Provider on-disk storage root.
//! For providers that need to persist session files, OAuth tokens etc.
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("could not determine the system config directory")]
    UnknownStorageDirectory,
}

#[derive(Debug, Clone)]
pub struct ProviderStorage {
    root: PathBuf,
}

impl ProviderStorage {
    // For tests:
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve the storage root for a provider.
    pub fn for_provider(name: &str) -> Result<Self, StorageError> {
        if let Ok(dir) = std::env::var("CALDIR_PROVIDER_STORAGE_DIR") {
            return Ok(Self::new(dir));
        }

        Ok(Self::new(default_root(name)?))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn default_root(provider_name: &str) -> Result<PathBuf, StorageError> {
    Ok(crate::utils::paths::platform_config_dir()
        .ok_or(StorageError::UnknownStorageDirectory)?
        .join("caldir")
        .join("providers")
        .join(provider_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_provided_root() {
        let s = ProviderStorage::new("/tmp/foo");
        assert_eq!(s.root(), Path::new("/tmp/foo"));
    }

    #[test]
    fn default_root_includes_provider_name_under_caldir_providers() {
        let path = default_root("hooli").unwrap();
        // Last three components are caldir/providers/{name}
        let suffix: PathBuf = path
            .components()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        assert_eq!(suffix, PathBuf::from("caldir/providers/hooli"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn default_root_uses_xdg_config_on_linux() {
        let expected_parent = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"));

        let path = default_root("hooli").unwrap();
        assert_eq!(path, expected_parent.join("caldir/providers/hooli"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn default_root_uses_home_dot_config_on_macos() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let path = default_root("hooli").unwrap();
        assert_eq!(path, home.join(".config/caldir/providers/hooli"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn default_root_uses_appdata_on_windows() {
        let appdata = PathBuf::from(std::env::var("APPDATA").unwrap());
        let path = default_root("hooli").unwrap();
        assert_eq!(path, appdata.join("caldir/providers/hooli"));
    }
}
