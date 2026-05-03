//! Credential storage for iCloud CalDAV authentication.
//!
//! Stores Apple ID + app-specific password under `{provider_dir}/session/`.

use anyhow::{Context, Result};
use caldir_core::remote::protocol::ProviderRequestContext;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// iCloud CalDAV session (credentials + discovered URLs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub apple_id: String,
    pub app_password: String,
    /// User-specific CalDAV principal URL (discovered during auth)
    pub principal_url: String,
    /// Calendar home URL (discovered during auth)
    pub calendar_home_url: String,
}

impl Session {
    fn path_for_apple_id(context: &ProviderRequestContext, apple_id: &str) -> PathBuf {
        let slug = apple_id.replace(['/', '\\', ':', '@', '.'], "_");
        context
            .provider_dir
            .join("session")
            .join(format!("{}.toml", slug))
    }

    fn path(&self, context: &ProviderRequestContext) -> PathBuf {
        Self::path_for_apple_id(context, &self.apple_id)
    }

    pub fn new(
        apple_id: impl Into<String>,
        app_password: impl Into<String>,
        principal_url: impl Into<String>,
        calendar_home_url: impl Into<String>,
    ) -> Self {
        Session {
            apple_id: apple_id.into(),
            app_password: app_password.into(),
            principal_url: principal_url.into(),
            calendar_home_url: calendar_home_url.into(),
        }
    }

    pub fn load(context: &ProviderRequestContext, apple_id: &str) -> Result<Self> {
        let path = Self::path_for_apple_id(context, apple_id);

        if !path.exists() {
            anyhow::bail!("iCloud session for {} not found!", apple_id);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read iCloud session from {}", path.display()))?;

        let session: Session = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse iCloud session from {}", path.display()))?;

        Ok(session)
    }

    pub fn save(&self, context: &ProviderRequestContext) -> Result<()> {
        let path = self.path(context);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create session directory: {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(&self).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        // Set to owner-only (0600) since file contains credentials
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Get credentials as (username, password) tuple for HTTP basic auth
    pub fn credentials(&self) -> (&str, &str) {
        (&self.apple_id, &self.app_password)
    }
}
