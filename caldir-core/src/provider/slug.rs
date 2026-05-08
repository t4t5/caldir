use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
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

pub const PROVIDER_BINARY_PREFIX: &str = "caldir-provider-";

pub fn provider_slug_from_filename(filename: &str) -> Option<ProviderSlug> {
    let slug = filename.strip_prefix(PROVIDER_BINARY_PREFIX)?;
    let slug = slug.strip_suffix(std::env::consts::EXE_SUFFIX)?;

    if slug.is_empty() {
        return None;
    }

    Some(ProviderSlug::from(slug))
}
