//! `Session` value type for generic CalDAV authentication.

use serde::{Deserialize, Serialize};

/// Generic CalDAV session: credentials + discovered URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub server_url: String,
    pub username: String,
    pub password: String,
    /// User-specific CalDAV principal URL (discovered during auth)
    pub principal_url: String,
    /// Calendar home URL (discovered during auth)
    pub calendar_home_url: String,
}

impl Session {
    pub fn new(
        server_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        principal_url: impl Into<String>,
        calendar_home_url: impl Into<String>,
    ) -> Self {
        Session {
            server_url: server_url.into(),
            username: username.into(),
            password: password.into(),
            principal_url: principal_url.into(),
            calendar_home_url: calendar_home_url.into(),
        }
    }

    /// Derive a slug from username and server host for use as a filename.
    pub(super) fn slug(username: &str, server_url: &str) -> String {
        let host = url::Url::parse(server_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let raw = format!("{}@{}", username, host);
        raw.replace(['/', '\\', ':', '@', '.'], "_")
    }

    /// Build an account identifier like "user@host".
    pub fn account_identifier(username: &str, server_url: &str) -> String {
        let host = url::Url::parse(server_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        format!("{}@{}", username, host)
    }

    /// Get credentials as `(username, password)` for HTTP basic auth.
    pub fn credentials(&self) -> (&str, &str) {
        (&self.username, &self.password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_filesystem_safe() {
        let slug = Session::slug("alice@example.com", "https://caldav.fastmail.com/");
        assert!(!slug.contains(['/', '\\', ':', '@', '.']));
        assert!(slug.contains("alice"));
        assert!(slug.contains("fastmail"));
    }

    #[test]
    fn account_identifier_uses_user_at_host_form() {
        let id =
            Session::account_identifier("alice@example.com", "https://caldav.fastmail.com/dav/");
        assert_eq!(id, "alice@example.com@caldav.fastmail.com");
    }

    #[test]
    fn account_identifier_falls_back_when_host_unparseable() {
        let id = Session::account_identifier("alice", "not a url");
        assert_eq!(id, "alice@unknown");
    }
}
