pub mod connect;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

use anyhow::Result;
use caldir_core::provider::ProviderStorage;

use crate::constants::PROVIDER_NAME;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::SessionStore;

async fn credentials(config: &ICloudRemoteConfig) -> Result<(String, String)> {
    resolve_credentials(&ProviderStorage::for_provider(PROVIDER_NAME)?, config).await
}

/// `(apple_id, password)` from `icloud_password_command`, else the stored session.
async fn resolve_credentials(
    storage: &ProviderStorage,
    config: &ICloudRemoteConfig,
) -> Result<(String, String)> {
    if let Some(command) = &config.icloud_password_command {
        return Ok((config.icloud_account.clone(), command.run().await?));
    }

    let session = SessionStore::new(storage.clone()).load(&config.icloud_account)?;
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
    use caldir_core::RemoteConfigParams;
    use tempfile::TempDir;

    const APPLE_ID: &str = "me@icloud.com";

    fn storage_with_session() -> (TempDir, ProviderStorage) {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        let session = Session::new(APPLE_ID, "stored", "https://p/", "https://h/");
        SessionStore::new(storage.clone()).save(&session).unwrap();
        (tmp, storage)
    }

    fn config(extra: &str) -> ICloudRemoteConfig {
        let params: RemoteConfigParams = toml::from_str(&format!(
            "icloud_account = \"{APPLE_ID}\"\nicloud_calendar_url = \"https://cal/\"\n{extra}"
        ))
        .unwrap();
        ICloudRemoteConfig::try_from(&params).unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_wins_over_session() {
        let (_tmp, storage) = storage_with_session();
        let config = config(r#"icloud_password_command = ["printf", "from-command"]"#);
        let creds = resolve_credentials(&storage, &config).await.unwrap();
        assert_eq!(creds, (APPLE_ID.to_string(), "from-command".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn password_command_works_without_session() {
        let tmp = TempDir::new().unwrap();
        let storage = ProviderStorage::new(tmp.path());
        let config = config(r#"icloud_password_command = ["printf", "from-command"]"#);
        let creds = resolve_credentials(&storage, &config).await.unwrap();
        assert_eq!(creds.1, "from-command");
    }

    #[tokio::test]
    async fn falls_back_to_session() {
        let (_tmp, storage) = storage_with_session();
        let creds = resolve_credentials(&storage, &config("")).await.unwrap();
        assert_eq!(creds, (APPLE_ID.to_string(), "stored".to_string()));
    }
}
