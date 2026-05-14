//! `Session` value type for iCloud CalDAV authentication.

use serde::{Deserialize, Serialize};

/// iCloud CalDAV session: credentials + discovered URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub apple_id: String,
    pub app_password: String,
    /// User-specific CalDAV principal URL (discovered during auth)
    pub principal_url: String,
    /// Calendar home URL (discovered during auth)
    pub calendar_home_url: String,
}

impl Session {
    pub fn new(
        apple_id: impl Into<String>,
        app_password: impl Into<String>,
        principal_url: impl Into<String>,
        calendar_home_url: impl Into<String>,
    ) -> Self {
        Session {
            apple_id: apple_id.into(),
            app_password: app_password.into(),
            principal_url: principal_url.into(),
            calendar_home_url: calendar_home_url.into(),
        }
    }

    /// Derive a slug from the Apple ID for use as a filename.
    ///
    /// Preserved byte-for-byte from the pre-migration iCloud provider so
    /// existing on-disk session files keep loading.
    pub(super) fn slug(apple_id: &str) -> String {
        apple_id.replace(['/', '\\', ':', '@', '.'], "_")
    }

    /// Get credentials as `(username, password)` for HTTP basic auth.
    pub fn credentials(&self) -> (&str, &str) {
        (&self.apple_id, &self.app_password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_strips_unsafe_chars() {
        let slug = Session::slug("alice@icloud.com");
        assert!(!slug.contains(['/', '\\', ':', '@', '.']));
        assert_eq!(slug, "alice_icloud_com");
    }

    #[test]
    fn slug_is_deterministic_for_apple_id() {
        let a = Session::slug("me@me.com");
        let b = Session::slug("me@me.com");
        assert_eq!(a, b);
    }
}
