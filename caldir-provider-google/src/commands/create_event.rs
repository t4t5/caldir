use anyhow::{Context, Result};
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::CreateEvent;
use google_calendar::types::SendUpdates;

use crate::app_config::AppConfigStore;
use crate::commands::invite::patch_invite_personal_fields;
use crate::commands::update_event::patch_event_without_attendees;
use crate::constants::PROVIDER_NAME;
use crate::event_id::override_instance_id;
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;
    let client = session_store.client(&session, &app_config_store)?;

    // Recurring instance override:
    // Shares the master's iCalUID, so creating via events().insert() trips Google's "duplicate identifier" check.
    // Google's data model treats an override as a modification of an existing auto-expanded instance.
    // PATCH the synthetic instance id `{master_id}_{rid}` instead.
    if let Some(rid) = cmd.event.recurrence_id.as_ref() {
        let instance_id = override_instance_id(&cmd.event, rid.as_event_time())?;

        // Invitations only update our response and personal reminders.
        if cmd.event.is_invite_for(account_email) {
            let google_event = patch_invite_personal_fields(
                &session,
                calendar_id,
                &instance_id,
                &cmd.event,
                account_email,
            )
            .await?;

            return Event::from_google(google_event);
        } else {
            let google_event = patch_event_without_attendees(
                session.access_token(),
                calendar_id,
                &instance_id,
                &cmd.event,
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to update recurring instance: {}",
                    cmd.event.summary.as_deref().unwrap_or_default()
                )
            })?;

            return Event::from_google(google_event);
        }
    }

    // Let google change the ID
    // (Otherwise we'll get "Invalid resource id value")
    let mut google_event = cmd.event.to_google();
    google_event.id = String::new();

    let response = match client
        .events()
        .insert(
            calendar_id,
            1,
            0,
            false,
            SendUpdates::All,
            false,
            &google_event,
        )
        .await
    {
        Ok(response) => Ok(response),
        Err(error)
            if google_event.conference_data.is_some() && is_conference_data_error(&error) =>
        {
            eprintln!(
                "caldir-provider-google: warning: Google rejected conference data; \
                 retrying without it: {error}"
            );
            google_event.conference_data = None;

            client
                .events()
                .insert(
                    calendar_id,
                    1,
                    0,
                    false,
                    SendUpdates::All,
                    false,
                    &google_event,
                )
                .await
        }
        Err(error) => Err(error),
    }
    .with_context(|| format!("Failed to create event: {}", google_event.summary))?;

    let created_event = Event::from_google(response.body)?;

    Ok(created_event)
}

fn is_conference_data_error(error: &google_calendar::ClientError) -> bool {
    match error {
        google_calendar::ClientError::HttpError { error, .. } => {
            error.to_ascii_lowercase().contains("conference")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_calendar::{ClientError, HeaderMap, StatusCode};

    fn http_error(message: &str) -> ClientError {
        ClientError::HttpError {
            status: StatusCode::BAD_REQUEST,
            headers: HeaderMap::new(),
            error: message.to_string(),
        }
    }

    #[test]
    fn identifies_conference_data_errors_for_insert_retry() {
        let error = http_error(r#"{"error":{"message":"Invalid conferenceData value"}}"#);

        assert!(is_conference_data_error(&error));
    }

    #[test]
    fn does_not_retry_unrelated_insert_errors() {
        let error = http_error(r#"{"error":{"message":"Invalid attendee email"}}"#);

        assert!(!is_conference_data_error(&error));
    }
}
