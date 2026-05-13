//! Filesystem-backed storage for [`Session`] credentials.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use std::path::PathBuf;

use super::Session;

/// Reads and writes [`Session`] files under a provider's storage root.
///
/// Layout: `{storage.root()}/session/{slug}.toml`, with the slug derived from
/// the session's username + server host. Session files contain plaintext
/// credentials; on Unix they're chmod'd to `0600`.
pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self {
        Self { storage }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.path_for(&session.username, &session.server_url);

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

    /// Find a session by its `account_identifier()` form ("user@host").
    ///
    /// Scans the session directory rather than computing the filename
    /// directly, since the on-disk slug encoding (`.` → `_`) is one-way.
    pub fn load(&self, account_identifier: &str) -> Result<Session> {
        let session_dir = self.session_dir();
        if !session_dir.exists() {
            anyhow::bail!("CalDAV session for {} not found!", account_identifier);
        }

        for entry in std::fs::read_dir(&session_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                let contents = std::fs::read_to_string(&path)?;
                if let Ok(session) = toml::from_str::<Session>(&contents) {
                    let id = Session::account_identifier(&session.username, &session.server_url);
                    if id == account_identifier {
                        return Ok(session);
                    }
                }
            }
        }

        anyhow::bail!("CalDAV session for {} not found!", account_identifier);
    }

    fn session_dir(&self) -> PathBuf {
        self.storage.root().join("session")
    }

    fn path_for(&self, username: &str, server_url: &str) -> PathBuf {
        self.session_dir()
            .join(format!("{}.toml", Session::slug(username, server_url)))
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
            Session::slug(&session.username, &session.server_url)
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

    #[cfg(unix)]
    #[test]
    fn save_chmods_session_file_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, store) = store();
        let session = sample_session();
        store.save(&session).unwrap();

        let path = tmp.path().join("session").join(format!(
            "{}.toml",
            Session::slug(&session.username, &session.server_url)
        ));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
