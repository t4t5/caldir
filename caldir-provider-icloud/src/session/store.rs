//! Filesystem-backed storage for [`Session`] credentials.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use std::path::PathBuf;

use super::Session;

/// Reads and writes [`Session`] files under a provider's storage root.
///
/// Layout: `{storage.root()}/session/{slug}.toml`, with the slug derived
/// from the Apple ID (the account identifier). Session files contain
/// plaintext credentials; on Unix they're chmod'd to `0600`.
pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.path_for(&session.apple_id);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create session directory: {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(session).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        // Plaintext credentials — owner-only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Load a session by its `account_identifier` (= Apple ID).
    ///
    /// The slug is forward-deterministic from the Apple ID, so we compute
    /// the path directly rather than scanning the directory.
    pub fn load(&self, account_identifier: &str) -> Result<Session> {
        let path = self.path_for(account_identifier);

        if !path.exists() {
            anyhow::bail!("iCloud session for {} not found!", account_identifier);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read iCloud session from {}", path.display()))?;

        let session: Session = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse iCloud session from {}", path.display()))?;

        Ok(session)
    }

    fn session_dir(&self) -> PathBuf {
        self.storage.root().join("session")
    }

    fn path_for(&self, apple_id: &str) -> PathBuf {
        self.session_dir()
            .join(format!("{}.toml", Session::slug(apple_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store() -> (TempDir, SessionStore) {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(ProviderStorage::new(tmp.path()));
        (tmp, store)
    }

    fn sample_session() -> Session {
        Session::new(
            "alice@icloud.com",
            "abcd-efgh-ijkl-mnop",
            "https://p01-caldav.icloud.com/123456/principal/",
            "https://p01-caldav.icloud.com/123456/calendars/",
        )
    }

    #[test]
    fn save_writes_toml_under_session_subdir() {
        let (tmp, store) = store();
        let session = sample_session();

        store.save(&session).unwrap();

        let expected = tmp
            .path()
            .join("session")
            .join(format!("{}.toml", Session::slug(&session.apple_id)));
        assert!(
            expected.is_file(),
            "session file should exist at {expected:?}"
        );
    }

    #[test]
    fn load_round_trips_by_apple_id() {
        let (_tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let loaded = store.load(&session.apple_id).unwrap();

        assert_eq!(loaded.apple_id, session.apple_id);
        assert_eq!(loaded.app_password, session.app_password);
        assert_eq!(loaded.principal_url, session.principal_url);
        assert_eq!(loaded.calendar_home_url, session.calendar_home_url);
    }

    #[test]
    fn load_errors_when_missing() {
        let (_tmp, store) = store();
        let err = store.load("ghost@icloud.com").unwrap_err();
        assert!(err.to_string().contains("ghost@icloud.com"));
    }

    #[cfg(unix)]
    #[test]
    fn save_chmods_session_file_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let path = tmp
            .path()
            .join("session")
            .join(format!("{}.toml", Session::slug(&session.apple_id)));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
