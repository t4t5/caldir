use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::DeleteEvent;
use google_calendar::types::SendUpdates;
use google_calendar::{ClientError, StatusCode};

use crate::app_config::AppConfigStore;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let google_event_id = cmd
        .event
        .x_property(PROVIDER_EVENT_ID_PROPERTY)
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot delete event without {PROVIDER_EVENT_ID_PROPERTY}")
        })?;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;
    let client = session_store.client(&session, &app_config_store)?;

    match client
        .events()
        .delete(calendar_id, google_event_id, false, SendUpdates::All)
        .await
    {
        Ok(_) => Ok(()),
        Err(error) if is_already_deleted_error(&error) => Ok(()),
        Err(error) => Err(error).context("Failed to delete event"),
    }
}

fn is_already_deleted_error(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::HttpError {
            status: StatusCode::GONE | StatusCode::NOT_FOUND,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_calendar::HeaderMap;

    fn http_error(status: StatusCode) -> ClientError {
        ClientError::HttpError {
            status,
            headers: HeaderMap::new(),
            error: String::new(),
        }
    }

    #[test]
    fn gone_event_is_already_deleted() {
        assert!(is_already_deleted_error(&http_error(StatusCode::GONE)));
    }

    #[test]
    fn missing_event_is_already_deleted() {
        assert!(is_already_deleted_error(&http_error(StatusCode::NOT_FOUND)));
    }

    #[test]
    fn other_http_errors_are_not_already_deleted() {
        assert!(!is_already_deleted_error(&http_error(
            StatusCode::FORBIDDEN
        )));
    }
}
