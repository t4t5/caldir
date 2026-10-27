pub mod connect;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;

use crate::constants::PROVIDER_NAME;
use crate::remote_config::CaldavRemoteConfig;
use crate::session::SessionStore;

async fn credentials(config: &CaldavRemoteConfig) -> Result<(String, String)> {
    resolve_credentials(&ProviderStorage::for_provider(PROVIDER_NAME)?, config).await
}

/// `(username, password)` from `caldav_password_command`, else the stored session.
async fn resolve_credentials(
    storage: &ProviderStorage,
    config: &CaldavRemoteConfig,
) -> Result<(String, String)> {
    if let Some(command) = &config.caldav_password_command {
        let username = config
            .caldav_username
            .clone()
            .context("`caldav_password_command` requires `caldav_username`")?;
        return Ok((username, command.run().await?));
    }

    let account = config.caldav_account.as_deref().context(
        "CalDAV remote configuration needs `caldav_account` or `caldav_password_command`",
    )?;
    let session = SessionStore::new(storage.clone()).load(account)?;
    Ok((session.username, session.password))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use caldir_core::RemoteConfigParams;
    use tempfile::TempDir;

    fn storage_with_session() -> (TempDir, ProviderStorage) {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        let session = Session::new(
            "https://caldav.fastmail.com/",
            "alice@example.com",
            "stored",
            "https://p/",
            "https://h/",
        );
        SessionStore::new(storage.clone()).save(&session).unwrap();
        (tmp, storage)
    }

    fn config(extra: &str) -> CaldavRemoteConfig {
        let params: RemoteConfigParams =
            toml::from_str(&format!("caldav_calendar_url = \"https://cal/\"\n{extra}")).unwrap();
        CaldavRemoteConfig::try_from(&params).unwrap()
    }

    const ACCOUNT: &str = r#"caldav_account = "alice@example.com@caldav.fastmail.com""#;
    const COMMAND: &str = r#"
caldav_username = "alice@example.com"
caldav_password_command = ["printf", "from-command"]
"#;

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_wins_over_session() {
        let (_tmp, storage) = storage_with_session();
        let config = config(&format!("{ACCOUNT}\n{COMMAND}"));
        let creds = resolve_credentials(&storage, &config).await.unwrap();
        assert_eq!(
            creds,
            ("alice@example.com".to_string(), "from-command".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_works_without_account_or_session() {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        let creds = resolve_credentials(&storage, &config(COMMAND))
            .await
            .unwrap();
        assert_eq!(creds.1, "from-command");
    }

    #[tokio::test]
    async fn password_command_requires_username() {
        let (_tmp, storage) = storage_with_session();
        let config = config(r#"caldav_password_command = ["printf", "x"]"#);
        let err = resolve_credentials(&storage, &config).await.unwrap_err();
        assert!(err.to_string().contains("caldav_username"));
    }

    #[tokio::test]
    async fn falls_back_to_session() {
        let (_tmp, storage) = storage_with_session();
        let creds = resolve_credentials(&storage, &config(ACCOUNT))
            .await
            .unwrap();
        assert_eq!(
            creds,
            ("alice@example.com".to_string(), "stored".to_string())
        );
    }

    #[tokio::test]
    async fn errors_without_account_or_command() {
        let (_tmp, storage) = storage_with_session();
        let err = resolve_credentials(&storage, &config(""))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("caldav_account"));
    }
}
