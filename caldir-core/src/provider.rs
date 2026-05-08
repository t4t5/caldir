mod error;
mod slug;

use error::ProviderError;
use std::path::{Path, PathBuf};

pub use slug::ProviderSlug;

pub struct Provider {
    slug: ProviderSlug,
    bin_path: PathBuf,
}

impl Provider {
    pub fn from_binary_path(binary_path: PathBuf) -> Result<Self, ProviderError> {
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
    use super::*;

    fn create_provider_binary(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(format!("{}{}", name, std::env::consts::EXE_SUFFIX));

        std::fs::write(&path, b"").unwrap();

        // Set executable permissions to executable:
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }

        path
    }

    #[test]
    fn from_binary_path_succeeds_for_valid_provider_binary() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = create_provider_binary(tmp.path(), "caldir-provider-hooli");

        let provider = Provider::from_binary_path(bin.clone()).unwrap();

        assert_eq!(provider.slug.as_str(), "hooli");
        assert_eq!(provider.bin_path, bin);
    }

    #[test]
    fn from_binary_path_errors_when_file_does_not_exist() {
        let tmp = tempfile::TempDir::new().unwrap();

        let bin = tmp.path().join(format!(
            "caldir-provider-hooli{}",
            std::env::consts::EXE_SUFFIX
        ));

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
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = create_provider_binary(tmp.path(), "hooli");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }

    #[test]
    fn from_binary_path_errors_when_slug_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = create_provider_binary(tmp.path(), "caldir-provider-");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }
}
