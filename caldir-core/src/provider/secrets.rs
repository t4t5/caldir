//! Storage for provider credentials (tokens, passwords, key material).
//!
//! Providers address secrets by key and never see where they live, so the
//! backend can move to an OS keyring without provider changes.

use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecretError {
    #[error("invalid secret key `{0}`")]
    InvalidKey(String),

    #[error("failed to read secret `{key}`")]
    Read {
        key: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write secret `{key}`")]
    Write {
        key: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to delete secret `{key}`")]
    Delete {
        key: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize secret `{key}`")]
    Serialize {
        key: String,
        #[source]
        source: toml::ser::Error,
    },

    #[error("failed to parse secret `{key}`")]
    Deserialize {
        key: String,
        #[source]
        source: toml::de::Error,
    },
}

/// Typed access to a provider's secrets, stored as owner-only TOML files at
/// `{root}/{key}.toml`.
///
/// Keys are `/`-separated names such as `session/alice@example.com`.
#[derive(Debug, Clone)]
pub struct SecretStore {
    root: PathBuf,
}

impl SecretStore {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, SecretError> {
        let Some(contents) = self.read(key)? else {
            return Ok(None);
        };
        toml::from_str(&contents)
            .map(Some)
            .map_err(|source| SecretError::Deserialize {
                key: key.to_string(),
                source,
            })
    }

    // `atomic_write` stages in a `NamedTempFile`, which is created 0600 on Unix.
    pub fn save<T: Serialize>(&self, key: &str, value: &T) -> Result<(), SecretError> {
        let path = self.path(key)?;
        let contents = toml::to_string_pretty(value).map_err(|source| SecretError::Serialize {
            key: key.to_string(),
            source,
        })?;
        crate::utils::atomic_write(&path, contents.as_bytes()).map_err(|err| SecretError::Write {
            key: key.to_string(),
            source: with_path(err, &path),
        })
    }

    pub fn exists(&self, key: &str) -> Result<bool, SecretError> {
        Ok(self.read(key)?.is_some())
    }

    /// Deleting a missing secret is not an error.
    pub fn delete(&self, key: &str) -> Result<(), SecretError> {
        let path = self.path(key)?;
        match std::fs::remove_file(&path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(SecretError::Delete {
                key: key.to_string(),
                source: with_path(err, &path),
            }),
            _ => Ok(()),
        }
    }

    fn read(&self, key: &str) -> Result<Option<String>, SecretError> {
        let path = self.path(key)?;
        match std::fs::read_to_string(&path) {
            Ok(contents) => Ok(Some(contents)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(SecretError::Read {
                key: key.to_string(),
                source: with_path(err, &path),
            }),
        }
    }

    fn path(&self, key: &str) -> Result<PathBuf, SecretError> {
        validate_key(key)?;
        Ok(self.root.join(format!("{key}.toml")))
    }
}

fn validate_key(key: &str) -> Result<(), SecretError> {
    let valid = key.split('/').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['\\', '\0'])
    });
    if valid {
        Ok(())
    } else {
        Err(SecretError::InvalidKey(key.to_string()))
    }
}

fn with_path(err: std::io::Error, path: &Path) -> std::io::Error {
    std::io::Error::new(err.kind(), format!("{}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Session {
        token: String,
    }

    fn store() -> (tempfile::TempDir, SecretStore) {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::new(tmp.path());
        (tmp, store)
    }

    fn session(token: &str) -> Session {
        Session {
            token: token.to_string(),
        }
    }

    #[test]
    fn round_trips_and_replaces() {
        let (_tmp, store) = store();

        store.save("session/alice", &session("one")).unwrap();
        store.save("session/alice", &session("two")).unwrap();

        assert_eq!(
            store.load::<Session>("session/alice").unwrap(),
            Some(session("two"))
        );
    }

    #[test]
    fn missing_secret_loads_as_none() {
        let (_tmp, store) = store();

        assert_eq!(store.load::<Session>("session/ghost").unwrap(), None);
        assert!(!store.exists("session/ghost").unwrap());
    }

    #[test]
    fn delete_removes_and_tolerates_missing() {
        let (_tmp, store) = store();
        store.save("pending", &session("x")).unwrap();

        store.delete("pending").unwrap();
        store.delete("pending").unwrap();

        assert!(!store.exists("pending").unwrap());
    }

    #[test]
    fn file_layout_matches_key() {
        let (tmp, store) = store();

        store
            .save("session/alice@example.com", &session("x"))
            .unwrap();

        let path = tmp.path().join("session/alice@example.com.toml");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "token = \"x\"\n");
    }

    #[cfg(unix)]
    #[test]
    fn files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (tmp, store) = store();

        store.save("session/alice", &session("x")).unwrap();

        let path = tmp.path().join("session/alice.toml");
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn unparseable_secret_errors() {
        let (tmp, store) = store();
        std::fs::write(tmp.path().join("broken.toml"), "token = [").unwrap();

        let err = store.load::<Session>("broken").unwrap_err();

        assert!(matches!(err, SecretError::Deserialize { key, .. } if key == "broken"));
    }

    #[test]
    fn rejects_keys_that_escape_or_are_ambiguous() {
        let (_tmp, store) = store();

        for key in ["", "/abs", "a//b", "a/", "../up", "a/./b", "a\\b"] {
            assert!(
                matches!(
                    store.save(key, &session("x")),
                    Err(SecretError::InvalidKey(_))
                ),
                "{key:?} should be rejected"
            );
        }
    }
}
