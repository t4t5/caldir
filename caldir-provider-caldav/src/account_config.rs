//! Per-account settings from the provider's `config.toml`, e.g.
//!
//! ```toml
//! [accounts."me@icloud.com"]
//! app_password_command = ["cat", "/run/secrets/icloud"]
//! ```
//!
//! Plain config, not secrets, so it can be written by hand or generated.

use std::collections::HashMap;
use std::io::ErrorKind;

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use serde::Deserialize;
use serde::de::DeserializeOwned;

const CONFIG_FILE: &str = "config.toml";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderConfig<T> {
    #[serde(default = "HashMap::new")]
    accounts: HashMap<String, T>,
}

/// Settings for `account`, or `None` if the file or the account entry is absent.
pub fn load_account_config<T: DeserializeOwned>(
    storage: &ProviderStorage,
    account: &str,
) -> Result<Option<T>> {
    let path = storage.root().join(CONFIG_FILE);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {}", path.display())),
    };

    let mut config: ProviderConfig<T> =
        toml::from_str(&contents).with_context(|| format!("Invalid {}", path.display()))?;
    Ok(config.accounts.remove(account))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Account {
        token: String,
    }

    fn storage_with(config: Option<&str>) -> (TempDir, ProviderStorage) {
        let tmp = TempDir::new().unwrap();
        if let Some(config) = config {
            std::fs::write(tmp.path().join(CONFIG_FILE), config).unwrap();
        }
        let storage = ProviderStorage::new(tmp.path());
        (tmp, storage)
    }

    #[test]
    fn missing_file_is_none() {
        let (_tmp, storage) = storage_with(None);
        let account = load_account_config::<Account>(&storage, "me@example.com").unwrap();
        assert_eq!(account, None);
    }

    #[test]
    fn missing_account_is_none() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"other@example.com\"]\ntoken = \"x\""));
        let account = load_account_config::<Account>(&storage, "me@example.com").unwrap();
        assert_eq!(account, None);
    }

    #[test]
    fn loads_account_by_identifier() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"me@example.com\"]\ntoken = \"x\""));
        let account = load_account_config::<Account>(&storage, "me@example.com").unwrap();
        assert_eq!(
            account,
            Some(Account {
                token: "x".to_string()
            })
        );
    }

    #[test]
    fn unknown_account_key_errors() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"me@example.com\"]\ntokn = \"x\""));
        let err = load_account_config::<Account>(&storage, "me@example.com").unwrap_err();
        assert!(err.to_string().contains(CONFIG_FILE));
    }
}
