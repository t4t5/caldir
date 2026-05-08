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
