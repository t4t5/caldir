//! Secret-backed storage for [`Session`] credentials.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;

use super::Session;

/// Reads and writes [`Session`]s as secrets keyed `session/{slug}`, the slug
/// derived from the account identifier ("user@host").
pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let account_identifier =
            Session::account_identifier(&session.username, &session.server_url);
        self.storage
            .secrets()
            .save(&Self::key(&account_identifier), session)?;
        Ok(())
    }

    /// Find a session by its `account_identifier()` form ("user@host").
    pub fn load(&self, account_identifier: &str) -> Result<Session> {
        self.storage
            .secrets()
            .load::<Session>(&Self::key(account_identifier))?
            // Distinct identifiers can share a slug; don't hand back the other account.
            .filter(|session| {
                Session::account_identifier(&session.username, &session.server_url)
                    == account_identifier
            })
            .with_context(|| format!("CalDAV session for {account_identifier} not found!"))
    }

    fn key(account_identifier: &str) -> String {
        format!("session/{}", Session::slug(account_identifier))
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
            "https://caldav.fastmail.com/",
            "alice@example.com",
            "secretpass",
            "/dav/principals/user/alice/",
            "/dav/calendars/alice/",
        )
    }

    #[test]
    fn save_writes_toml_under_session_subdir() {
        let (tmp, store) = store();
        let session = sample_session();

        store.save(&session).unwrap();

        let expected = tmp.path().join("session").join(format!(
            "{}.toml",
            Session::slug(&Session::account_identifier(
                &session.username,
                &session.server_url
            ))
        ));
        assert!(
            expected.is_file(),
            "session file should exist at {expected:?}"
        );
    }

    #[test]
    fn load_round_trips_by_account_identifier() {
        let (_tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let account_id = Session::account_identifier(&session.username, &session.server_url);
        let loaded = store.load(&account_id).unwrap();

        assert_eq!(loaded.server_url, session.server_url);
        assert_eq!(loaded.username, session.username);
        assert_eq!(loaded.password, session.password);
        assert_eq!(loaded.principal_url, session.principal_url);
        assert_eq!(loaded.calendar_home_url, session.calendar_home_url);
    }

    #[test]
    fn load_errors_when_no_session_directory() {
        let (_tmp, store) = store();
        let err = store
            .load("alice@example.com@caldav.fastmail.com")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("alice@example.com@caldav.fastmail.com")
        );
    }

    #[test]
    fn load_errors_when_account_not_found() {
        let (_tmp, store) = store();
        store.save(&sample_session()).unwrap();

        let err = store.load("ghost@example.com@nowhere").unwrap_err();
        assert!(err.to_string().contains("ghost@example.com@nowhere"));
    }

    #[test]
    fn load_rejects_other_account_sharing_the_slug() {
        let (_tmp, store) = store();
        store.save(&sample_session()).unwrap();

        // "alice_example_com@..." slugs the same as "alice@example.com@...".
        let err = store
            .load("alice_example_com@caldav.fastmail.com")
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[cfg(unix)]
    #[test]
    fn save_chmods_session_file_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let path = tmp.path().join("session").join(format!(
            "{}.toml",
            Session::slug(&Session::account_identifier(
                &session.username,
                &session.server_url
            ))
        ));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
