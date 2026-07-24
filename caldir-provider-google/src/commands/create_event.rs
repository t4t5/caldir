use anyhow::{Context, Result, anyhow};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::CreateEvent;
use caldir_core::{Event, EventTime};
use google_calendar::types::SendUpdates;

use crate::app_config::AppConfigStore;
use crate::commands::invite::patch_invite_status;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
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
    // PUT the synthetic instance id `{master_id}_{rid}` instead.
    if let Some(rid) = cmd.event.recurrence_id.as_ref() {
        let master_id = cmd
            .event
            .x_property(PROVIDER_EVENT_ID_PROPERTY)
            .ok_or_else(|| {
                anyhow!(
                    "Cannot create recurring instance override without master's \
                     {PROVIDER_EVENT_ID_PROPERTY}"
                )
            })?;

        let instance_id = format!(
            "{}_{}",
            master_id,
            google_instance_suffix(rid.as_event_time())
        );

        // If it's just an RSVP status update, use PATCH instead of PUT:
        if cmd.event.is_invite_for(account_email) {
            let google_event = patch_invite_status(
                &session,
                calendar_id,
                &instance_id,
                &cmd.event,
                account_email,
            )
            .await?;

            return Event::from_google(google_event);
        } else {
            let mut google_event = cmd.event.to_google();

            google_event.id = instance_id.clone();

            // Overrides never carry their own RRULE in Google's model.
            google_event.recurrence = Vec::new();

            let response = client
                .events()
                .update(
                    calendar_id,
                    &instance_id,
                    0,
                    0,
                    false,
                    SendUpdates::All,
                    false,
                    &google_event,
                )
                .await
                .with_context(|| {
                    format!(
                        "Failed to update recurring instance: {}",
                        google_event.summary
                    )
                })?;

            return Event::from_google(response.body);
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
                "caldir-provider-google: warning: Google rejected copied conference data; \
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

/// Format a `recurrence_id` as the suffix Google appends to a recurring
/// event's id to identify a single instance:
/// - all-day:    `YYYYMMDD`
/// - timed:      `YYYYMMDDTHHMMSSZ` (UTC)
///
/// Zoned instances get their wallclock resolved to a UTC instant so the
/// suffix matches what Google's API auto-expansion uses.
fn google_instance_suffix(rid: &EventTime) -> String {
    match rid {
        EventTime::Date(d) => d.format("%Y%m%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y%m%dT%H%M%SZ").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y%m%dT%H%M%SZ").to_string(),
        EventTime::DateTimeZoned { datetime, tzid } => {
            let utc = if let Ok(tz) = tzid.parse::<chrono_tz::Tz>() {
                datetime
                    .and_local_timezone(tz)
                    .single()
                    .map(|zoned| zoned.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|| datetime.and_utc())
            } else {
                datetime.and_utc()
            };
            utc.format("%Y%m%dT%H%M%SZ").to_string()
        }
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
