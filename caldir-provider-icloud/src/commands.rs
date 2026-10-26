pub mod connect;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

use anyhow::Result;
use caldir_core::provider::{PasswordCommand, ProviderStorage};
use serde::Deserialize;

use crate::constants::PROVIDER_NAME;
use crate::session::SessionStore;

/// `[accounts."{apple_id}"]` in the provider's `config.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountConfig {
    app_password_command: Option<PasswordCommand>,
}

async fn credentials(apple_id: &str) -> Result<(String, String)> {
    resolve_credentials(&ProviderStorage::for_provider(PROVIDER_NAME)?, apple_id).await
}

/// `(username, password)` from the account's `app_password_command`, else the stored session.
async fn resolve_credentials(
    storage: &ProviderStorage,
    apple_id: &str,
) -> Result<(String, String)> {
    let config = storage.account_config::<AccountConfig>(apple_id)?;
    if let Some(command) = config.and_then(|c| c.app_password_command) {
        return Ok((apple_id.to_string(), command.run().await?));
    }

    let session = SessionStore::new(storage.clone()).load(apple_id)?;
    Ok((session.apple_id, session.app_password))
}

fn normalize_event(mut event: caldir_core::Event) -> caldir_core::Event {
    // iCloud adds this query-window-dependent marker; RECURRENCE-ID carries identity.
    event
        .x_properties
        .retain(|p| !p.name.eq_ignore_ascii_case("X-RECURRENCE-EXCEPTION"));
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use tempfile::TempDir;

    const APPLE_ID: &str = "me@icloud.com";

    fn storage(config: Option<&str>, session: bool) -> (TempDir, ProviderStorage) {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        if let Some(config) = config {
            std::fs::write(tmp.path().join("config.toml"), config).unwrap();
        }
        if session {
            let session = Session::new(APPLE_ID, "stored", "https://p/", "https://h/");
            SessionStore::new(storage.clone()).save(&session).unwrap();
        }
        (tmp, storage)
    }

    const COMMAND_CONFIG: &str = r#"
[accounts."me@icloud.com"]
app_password_command = ["printf", "from-command"]
"#;

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_wins_over_session() {
        let (_tmp, storage) = storage(Some(COMMAND_CONFIG), true);
        let creds = resolve_credentials(&storage, APPLE_ID).await.unwrap();
        assert_eq!(creds, (APPLE_ID.to_string(), "from-command".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_works_without_session() {
        let (_tmp, storage) = storage(Some(COMMAND_CONFIG), false);
        let creds = resolve_credentials(&storage, APPLE_ID).await.unwrap();
        assert_eq!(creds.1, "from-command");
    }

    #[tokio::test]
    async fn falls_back_to_session() {
        let (_tmp, storage) = storage(None, true);
        let creds = resolve_credentials(&storage, APPLE_ID).await.unwrap();
        assert_eq!(creds, (APPLE_ID.to_string(), "stored".to_string()));
    }

    #[tokio::test]
    async fn rejects_unknown_account_key() {
        let config = "[accounts.\"me@icloud.com\"]\npassword_command = [\"cat\"]";
        let (_tmp, storage) = storage(Some(config), true);
        assert!(resolve_credentials(&storage, APPLE_ID).await.is_err());
    }
}
