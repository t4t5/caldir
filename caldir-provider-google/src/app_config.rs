//! App-level configuration for the Google provider.
//!
//! User-provided OAuth credentials stored at `{storage.root()}/app_config.toml`.
//! Only present when the user opts into self-hosted OAuth (`--hosted=false`).

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Google OAuth client credentials (user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: String,
}

/// Filesystem-backed storage for [`AppConfig`].
///
/// Layout: `{storage.root()}/app_config.toml`. Contains a client secret, so
/// chmod'd to `0600` on Unix.
pub struct AppConfigStore {
    storage: ProviderStorage,
}

impl AppConfigStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn exists(&self) -> bool {
        self.path().exists()
    }

    pub fn load(&self) -> Result<AppConfig> {
        let path = self.path();

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

    pub fn save(&self, app_config: &AppConfig) -> Result<()> {
        let path = self.path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let contents =
            toml::to_string_pretty(app_config).context("Failed to serialize app config")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write app_config to {}", path.display()))?;

        // Plaintext client secret — owner-only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    fn path(&self) -> PathBuf {
        self.storage.root().join("app_config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store() -> (TempDir, AppConfigStore) {
        let tmp = TempDir::new().unwrap();
        let store = AppConfigStore::new(ProviderStorage::new(tmp.path()));
        (tmp, store)
    }

    fn sample() -> AppConfig {
        AppConfig {
            client_id: "id.apps.googleusercontent.com".to_string(),
            client_secret: "secret".to_string(),
        }
    }

    #[test]
    fn save_writes_toml_under_storage_root() {
        let (tmp, store) = store();
        store.save(&sample()).unwrap();
        assert!(tmp.path().join("app_config.toml").is_file());
    }

    #[test]
    fn load_round_trips() {
        let (_tmp, store) = store();
        let cfg = sample();
        store.save(&cfg).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.client_id, cfg.client_id);
        assert_eq!(loaded.client_secret, cfg.client_secret);
    }

    #[test]
    fn load_errors_when_missing() {
        let (_tmp, store) = store();
        let err = store.load().unwrap_err();
        assert!(err.to_string().contains("Google app config not found"));
    }

    #[test]
    fn exists_reports_presence() {
        let (_tmp, store) = store();
        assert!(!store.exists());
        store.save(&sample()).unwrap();
        assert!(store.exists());
    }

    #[cfg(unix)]
    #[test]
    fn save_chmods_app_config_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, store) = store();
        store.save(&sample()).unwrap();

        let path = tmp.path().join("app_config.toml");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
