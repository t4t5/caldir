use super::error::ProviderError;
use crate::{Provider, ProviderSlug};
use std::collections::HashMap;
use std::path::Path;

use super::slug::PROVIDER_BINARY_PREFIX;

pub struct ProviderRegistry(HashMap<ProviderSlug, Provider>);

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

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

    /// Slugs of all registered providers. Order is not stable.
    pub fn slugs(&self) -> Vec<&ProviderSlug> {
        self.0.keys().collect()
    }

    fn from_dirs<I>(dirs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        let mut registry = Self::new();

        for dir in dirs {
            for provider in discover_providers_in(dir.as_ref()) {
                registry
                    .0
                    .entry(provider.slug().clone())
                    .or_insert(provider);
            }
        }

        registry
    }

    /// Add providers found in `dir`, overriding any with a conflicting slug.
    pub fn add_from_dir(&mut self, dir: impl AsRef<Path>) {
        for provider in discover_providers_in(dir.as_ref()) {
            self.add(provider);
        }
    }
}

fn has_provider_prefix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PROVIDER_BINARY_PREFIX))
}

// Find all provider binaries in a directory:
fn discover_providers_in(dir: &Path) -> impl Iterator<Item = Provider> {
    let entries = std::fs::read_dir(dir).into_iter().flatten();

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_provider_prefix(path))
        .filter_map(|path| Provider::from_binary_path(path).ok())
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

        let transport_debug = format!("{:?}", provider.transport());
        assert!(transport_debug.contains(bin_path_1.to_str().unwrap()));
    }

    #[test]
    fn add_from_dir_overrides_conflicting_path_providers() {
        let (path_dir, _) = test_binary("caldir-provider-hooli");
        let (bundled_dir, bundled_bin) = test_binary("caldir-provider-hooli");

        let mut registry = ProviderRegistry::from_dirs([path_dir.path()]);
        registry.add_from_dir(bundled_dir.path());

        let provider = registry.get(&ProviderSlug::from("hooli")).unwrap();
        let debug = format!("{:?}", provider.transport());
        assert!(debug.contains(bundled_bin.to_str().unwrap()));
    }

    #[test]
    fn add_overwrites_existing_provider_with_same_slug() {
        let (dir, bin_path) = test_binary("caldir-provider-hooli");
        let mut registry = ProviderRegistry::from_dirs([dir.path()]);
        {
            let retrieved = registry.get(&ProviderSlug::from("hooli")).unwrap();
            let debug = format!("{:?}", retrieved.transport());
            assert!(debug.contains(bin_path.to_str().unwrap()));
        }

        // Build a new provider pointing at a different binary, and add it
        // under the same slug. Use a binary whose filename still parses as
        // the "hooli" slug — `caldir-provider-hooli` in a fresh tempdir.
        let (_tmp_new, bin_path_new) = test_binary("caldir-provider-hooli");
        let provider_new = Provider::from_binary_path(bin_path_new.clone()).unwrap();
        registry.add(provider_new);

        let retrieved = registry.get(&ProviderSlug::from("hooli")).unwrap();
        let debug = format!("{:?}", retrieved.transport());
        assert!(debug.contains(bin_path_new.to_str().unwrap()));
    }
}
