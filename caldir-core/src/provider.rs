mod error;
mod registry;
mod slug;

use std::path::{Path, PathBuf};

pub(crate) use error::ProviderError;
pub use registry::ProviderRegistry;
pub use slug::ProviderSlug;

#[derive(Debug, Clone)]
pub struct Provider {
    slug: ProviderSlug,
    bin_path: PathBuf,
}

impl Provider {
    pub(crate) fn from_binary_path(binary_path: PathBuf) -> Result<Self, ProviderError> {
        if !is_executable(&binary_path) {
            return Err(ProviderError::NotExecutable(binary_path));
        }

        let slug = binary_path
            .file_name()
            .and_then(|filename| filename.to_str())
            .and_then(provider_slug_from_filename)
            .ok_or_else(|| ProviderError::InvalidProviderFilename(binary_path.clone()))?;

        Ok(Provider::new(slug, &binary_path))
    }

    fn new(slug: ProviderSlug, binary_path: &Path) -> Self {
        Provider {
            slug,
            bin_path: binary_path.into(),
        }
    }

    fn slug(&self) -> &ProviderSlug {
        &self.slug
    }

    fn bin_path(&self) -> &Path {
        &self.bin_path
    }
}

const PROVIDER_BINARY_PREFIX: &str = "caldir-provider-";

fn provider_slug_from_filename(filename: &str) -> Option<ProviderSlug> {
    let slug = filename.strip_prefix(PROVIDER_BINARY_PREFIX)?;
    let slug = slug.strip_suffix(std::env::consts::EXE_SUFFIX)?;

    if slug.is_empty() {
        return None;
    }

    Some(ProviderSlug::from(slug))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"))
            .is_some_and(|extension| extension.eq_ignore_ascii_case(std::env::consts::EXE_SUFFIX))
}

#[cfg(not(any(unix, windows)))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_binary;

    use super::*;

    #[test]
    fn from_binary_path_succeeds_for_valid_provider_binary() {
        let (_tmp, bin) = test_binary("caldir-provider-hooli");

        let provider = Provider::from_binary_path(bin.clone()).unwrap();

        assert_eq!(provider.slug.as_str(), "hooli");
        assert_eq!(provider.bin_path, bin);
    }

    #[test]
    fn from_binary_path_errors_when_file_does_not_exist() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("caldir-provider-nonexistant");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::NotExecutable(p)) if p == bin));
    }

    #[cfg(unix)]
    #[test]
    fn from_binary_path_errors_when_file_not_executable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("caldir-provider-hooli");
        std::fs::write(&bin, b"").unwrap();

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::NotExecutable(p)) if p == bin));
    }

    #[test]
    fn from_binary_path_errors_when_filename_lacks_prefix() {
        let (_tmp, bin) = test_binary("hooli");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }

    #[test]
    fn from_binary_path_errors_when_slug_is_empty() {
        let (_tmp, bin) = test_binary("caldir-provider");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }
}
