//! App-level configuration for the Outlook provider.
//!
//! User-provided Azure AD OAuth credentials stored at:
//!   ~/.config/caldir/providers/outlook/app_config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use caldir_core::caldir_config::CaldirConfig;

use crate::constants::PROVIDER_NAME;

pub fn base_dir() -> Result<PathBuf> {
    Ok(CaldirConfig::config_dir()
        .context("Could not determine caldir config directory")?
        .join("providers")
        .join(PROVIDER_NAME))
}

/// Azure AD OAuth client credentials (user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl AppConfig {
    fn path() -> Result<PathBuf> {
        Ok(base_dir()?.join("app_config.toml"))
    }

    pub fn exists() -> Result<bool> {
        Ok(Self::path()?.exists())
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;

        if !path.exists() {
            anyhow::bail!(
                "Outlook app config not found at {}. Run `caldir connect outlook` to set up.",
                path.display()
            );
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read app_config from {}", path.display()))?;

        let app_config: AppConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse app_config from {}", path.display()))?;

        Ok(app_config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;

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
