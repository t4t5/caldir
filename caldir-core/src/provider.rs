mod error;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fmt, path::PathBuf};

use error::ProviderError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderSlug(String);

impl ProviderSlug {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProviderSlug {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProviderSlug {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl fmt::Display for ProviderSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Provider {
    slug: ProviderSlug,
    bin_path: PathBuf,
}

impl Provider {
    pub fn from_binary_path(binary_path: PathBuf) -> Result<Self, ProviderError> {
        if !is_executable(&binary_path) {
            return Err(ProviderError::NotExecutable(binary_path));
        }

        let filename = binary_path
            .file_name()
            .ok_or_else(|| ProviderError::MissingFilename(binary_path.clone()))?
            .to_str()
            .ok_or_else(|| ProviderError::NonUtf8Filename(binary_path.clone()))?;

        let slug = provider_slug_from_filename(filename)
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
