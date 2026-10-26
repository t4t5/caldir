pub mod connect;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

use anyhow::Result;
use caldir_core::provider::ProviderStorage;
use caldir_provider_caldav::account_config::load_account_config;
use caldir_provider_caldav::password_command::PasswordCommand;
use serde::Deserialize;

use crate::constants::PROVIDER_NAME;
use crate::session::{Session, SessionStore};

/// `[accounts."{account}"]` in the provider's `config.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountConfig {
    password_command: Option<PasswordCommand>,
}

async fn credentials(account: &str) -> Result<(String, String)> {
    resolve_credentials(&ProviderStorage::for_provider(PROVIDER_NAME)?, account).await
}

/// `(username, password)` from the account's `password_command`, else the stored session.
async fn resolve_credentials(storage: &ProviderStorage, account: &str) -> Result<(String, String)> {
    let config = load_account_config::<AccountConfig>(storage, account)?;
    if let Some(command) = config.and_then(|c| c.password_command) {
        let username = Session::username_from_account_identifier(account);
        return Ok((username.to_string(), command.run().await?));
    }

    let session = SessionStore::new(storage.clone()).load(account)?;
    Ok((session.username, session.password))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const ACCOUNT: &str = "alice@example.com@caldav.fastmail.com";

    fn storage(config: Option<&str>, session: bool) -> (TempDir, ProviderStorage) {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        if let Some(config) = config {
            std::fs::write(tmp.path().join("config.toml"), config).unwrap();
        }
        if session {
            let session = Session::new(
                "https://caldav.fastmail.com/",
                "alice@example.com",
                "stored",
                "https://p/",
                "https://h/",
            );
            SessionStore::new(storage.clone()).save(&session).unwrap();
        }
        (tmp, storage)
    }

    const COMMAND_CONFIG: &str = r#"
[accounts."alice@example.com@caldav.fastmail.com"]
password_command = ["printf", "from-command"]
"#;

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_wins_over_session() {
        let (_tmp, storage) = storage(Some(COMMAND_CONFIG), true);
        let creds = resolve_credentials(&storage, ACCOUNT).await.unwrap();
        assert_eq!(
            creds,
            ("alice@example.com".to_string(), "from-command".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_works_without_session() {
        let (_tmp, storage) = storage(Some(COMMAND_CONFIG), false);
        let creds = resolve_credentials(&storage, ACCOUNT).await.unwrap();
        assert_eq!(creds.1, "from-command");
    }

    #[tokio::test]
    async fn falls_back_to_session() {
        let (_tmp, storage) = storage(None, true);
        let creds = resolve_credentials(&storage, ACCOUNT).await.unwrap();
        assert_eq!(
            creds,
            ("alice@example.com".to_string(), "stored".to_string())
        );
    }
}
