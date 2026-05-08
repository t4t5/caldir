use super::error::ProviderError;
use crate::{Provider, ProviderSlug};
use std::collections::HashMap;
use std::path::Path;

use super::slug::PROVIDER_BINARY_PREFIX;

pub struct ProviderRegistry(HashMap<ProviderSlug, Provider>);

impl ProviderRegistry {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Find all "caldir-provider-{xxx}" binaries in the system `PATH`:
    pub fn from_system_path() -> Self {
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        Self::from_dirs(std::env::split_paths(&path_var))
    }

    pub(crate) fn get(&self, slug: &ProviderSlug) -> Result<&Provider, ProviderError> {
        self.0
            .get(slug)
            .ok_or_else(|| ProviderError::ProviderNotFound(slug.to_string()))
    }

    pub fn add(&mut self, provider: Provider) {
        self.0.insert(provider.slug().clone(), provider);
    }

    fn from_dirs<I>(dirs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        let mut registry = Self::new();

        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(dir.as_ref()) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();

                if !has_provider_prefix(&path) {
                    continue;
                }

                let Ok(provider) = Provider::from_binary_path(path) else {
                    continue;
                };

                registry
                    .0
                    .entry(provider.slug().clone())
                    .or_insert(provider);
            }
        }

        registry
    }
}

fn has_provider_prefix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PROVIDER_BINARY_PREFIX))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    use crate::test_utils::test_binary;

    #[test]
    fn from_dirs_discovers_provider_binaries() {
        let (dir1, _) = test_binary("caldir-provider-hooli");
        let (dir2, _) = test_binary("caldir-provider-aviato");

        let registry = ProviderRegistry::from_dirs([dir1.path(), dir2.path()]);

        assert!(registry.get(&ProviderSlug::from("hooli")).is_ok());
        assert!(registry.get(&ProviderSlug::from("aviato")).is_ok());
    }

    #[test]
    fn from_dirs_ignores_files_without_provider_prefix() {
        let (dir1, _) = test_binary("ls");
        let (dir2, _) = test_binary("some-other-tool");

        let registry = ProviderRegistry::from_dirs([dir1.path(), dir2.path()]);

        assert!(registry.0.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn from_dirs_skips_non_executable_provider_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("caldir-provider-hooli"), b"").unwrap();

        let registry = ProviderRegistry::from_dirs([tmp.path().to_path_buf()]);

        assert!(registry.get(&ProviderSlug::from("hooli")).is_err());
    }

    #[test]
    fn from_dirs_skips_provider_binary_with_empty_slug() {
        let (dir, _) = test_binary("caldir-provider-");

        let registry = ProviderRegistry::from_dirs([dir.path()]);

        assert!(registry.0.is_empty());
    }

    #[test]
    fn from_dirs_continues_when_dir_does_not_exist() {
        let (dir, _) = test_binary("caldir-provider-hooli");

        let registry = ProviderRegistry::from_dirs([
            PathBuf::from("/nonexistent/path/that/does/not/exist"),
            dir.path().to_path_buf(),
        ]);

        assert!(registry.get(&ProviderSlug::from("hooli")).is_ok());
    }

    #[test]
    fn from_dirs_resolves_conflicting_provider_slugs_to_first() {
        // For PATH, we mimic how shells resolves binaries (first match wins)
        let (dir1, bin_path_1) = test_binary("caldir-provider-hooli");
        let (dir2, _bin_path_2) = test_binary("caldir-provider-hooli");

        let registry = ProviderRegistry::from_dirs([dir1.path(), dir2.path()]);

        let provider = registry.get(&ProviderSlug::from("hooli")).unwrap();

        assert_eq!(provider.bin_path(), bin_path_1);
    }

    #[test]
    fn add_overwrites_existing_provider_with_same_slug() {
        let (dir, bin_path) = test_binary("caldir-provider-hooli");
        let mut registry = ProviderRegistry::from_dirs([dir.path()]);
        let retrieved_provider = registry.get(&ProviderSlug::from("hooli")).unwrap();

        assert_eq!(retrieved_provider.bin_path(), bin_path);

        // Create new version of provider
        let (_, bin_path_new) = test_binary("caldir-provider-hooli-new");
        let provider_new = Provider::new(ProviderSlug::from("hooli"), &bin_path_new);

        // Add it to the registry
        registry.add(provider_new);

        let retrieved_provider = registry.get(&ProviderSlug::from("hooli")).unwrap();

        assert_eq!(retrieved_provider.bin_path(), bin_path_new);
    }
}
