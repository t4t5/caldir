//! Per-account settings in the provider's `config.toml`, under
//! `[accounts."{account}"]`. Plain config, not secrets.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

pub(super) const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AccountConfigError {
    #[error("failed to read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid {}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

#[derive(Deserialize)]
struct ProviderConfig<T> {
    #[serde(default = "HashMap::new")]
    accounts: HashMap<String, T>,
}

/// Settings for `account`, or `None` if the file or the account entry is absent.
pub(super) fn load<T: DeserializeOwned>(
    root: &Path,
    account: &str,
) -> Result<Option<T>, AccountConfigError> {
    let path = root.join(CONFIG_FILE);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(AccountConfigError::Read { path, source }),
    };

    let mut config: ProviderConfig<T> =
        toml::from_str(&contents).map_err(|source| AccountConfigError::Parse { path, source })?;
    Ok(config.accounts.remove(account))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderStorage;
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
        let account = storage.account_config::<Account>("me@example.com").unwrap();
        assert_eq!(account, None);
    }

    #[test]
    fn missing_account_is_none() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"other@example.com\"]\ntoken = \"x\""));
        let account = storage.account_config::<Account>("me@example.com").unwrap();
        assert_eq!(account, None);
    }

    #[test]
    fn loads_account_by_identifier() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"me@example.com\"]\ntoken = \"x\""));
        let account = storage.account_config::<Account>("me@example.com").unwrap();
        assert_eq!(
            account,
            Some(Account {
                token: "x".to_string()
            })
        );
    }

    #[test]
    fn ignores_unknown_top_level_keys() {
        let (_tmp, storage) = storage_with(Some(
            "other = 1\n[accounts.\"me@example.com\"]\ntoken = \"x\"",
        ));
        let account = storage.account_config::<Account>("me@example.com").unwrap();
        assert!(account.is_some());
    }

    #[test]
    fn unknown_account_key_errors() {
        let (_tmp, storage) = storage_with(Some("[accounts.\"me@example.com\"]\ntokn = \"x\""));
        let err = storage
            .account_config::<Account>("me@example.com")
            .unwrap_err();
        assert!(matches!(err, AccountConfigError::Parse { .. }));
        assert!(err.to_string().contains(CONFIG_FILE));
    }
}
