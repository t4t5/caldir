//! Secret-backed storage for [`Session`] credentials.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;

use super::Session;

/// Reads and writes [`Session`]s as secrets keyed `session/{slug}`, the slug
/// derived from the Apple ID (the account identifier).
pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        self.storage
            .secrets()
            .save(&Self::key(&session.apple_id), session)?;
        Ok(())
    }

    /// Load a session by its `account_identifier` (= Apple ID).
    pub fn load(&self, account_identifier: &str) -> Result<Session> {
        self.storage
            .secrets()
            .load(&Self::key(account_identifier))?
            .with_context(|| format!("iCloud session for {account_identifier} not found!"))
    }

    fn key(apple_id: &str) -> String {
        format!("session/{}", Session::slug(apple_id))
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
