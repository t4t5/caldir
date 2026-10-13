//! App-level configuration for the Outlook provider.
//!
//! User-provided Azure AD OAuth credentials stored as the `app_config` secret.
//! Only present when the user opts into self-hosted OAuth (`--hosted=false`).

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use serde::{Deserialize, Serialize};

const KEY: &str = "app_config";

/// Azure AD OAuth client credentials (user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: String,
}

/// Secret-backed storage for [`AppConfig`], which contains a client secret.
pub struct AppConfigStore {
    storage: ProviderStorage,
}

impl AppConfigStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn exists(&self) -> Result<bool> {
        Ok(self.storage.secrets().exists(KEY)?)
    }

    pub fn load(&self) -> Result<AppConfig> {
        self.storage
            .secrets()
            .load(KEY)?
            .context("Outlook app config not found. Run `caldir connect outlook` to set up.")
    }

    pub fn save(&self, app_config: &AppConfig) -> Result<()> {
        self.storage.secrets().save(KEY, app_config)?;
        Ok(())
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
            client_id: "azure-app-id".to_string(),
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
        assert!(err.to_string().contains("Outlook app config not found"));
    }

    #[test]
    fn exists_reports_presence() {
        let (_tmp, store) = store();
        assert!(!store.exists().unwrap());
        store.save(&sample()).unwrap();
        assert!(store.exists().unwrap());
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
