//! App-level configuration for the Google provider.
//!
//! User-provided OAuth credentials stored at `{provider_dir}/app_config.toml`.

use anyhow::{Context, Result};
use caldir_core::remote::protocol::ProviderRequestContext;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Google OAuth client credentials (user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl AppConfig {
    fn path(context: &ProviderRequestContext) -> PathBuf {
        context.provider_dir.join("app_config.toml")
    }

    pub fn exists(context: &ProviderRequestContext) -> bool {
        Self::path(context).exists()
    }

    pub fn load(context: &ProviderRequestContext) -> Result<Self> {
        let path = Self::path(context);

        if !path.exists() {
            anyhow::bail!(
                "Google app config not found at {}. Run `caldir connect google` to set up.",
                path.display()
            );
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read app_config from {}", path.display()))?;

        let app_config: AppConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse app_config from {}", path.display()))?;

        Ok(app_config)
    }

    pub fn save(&self, context: &ProviderRequestContext) -> Result<()> {
        let path = Self::path(context);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize app config")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write app_config to {}", path.display()))?;

        // Set to owner-only (0600) since file contains OAuth client secret:
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }
}
