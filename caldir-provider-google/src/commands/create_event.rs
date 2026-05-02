use anyhow::{Context, Result, anyhow};
use caldir_core::event::{Event, EventTime};
use caldir_core::remote::protocol::CreateEvent;
use google_calendar::types::SendUpdates;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    // Recurring instance overrides share the master's iCalUID, so creating
    // them via events().insert() trips Google's "duplicate identifier" check.
    // Google's data model treats an override as a modification of an existing
    // auto-expanded instance: PUT the synthetic instance id `{master_id}_{rid}`
    // instead.
    if let Some(rid) = cmd.event.recurrence_id.as_ref() {
        let master_id = cmd
            .event
            .custom_property(PROVIDER_EVENT_ID_PROPERTY)
            .ok_or_else(|| {
                anyhow!(
                    "Cannot create recurring instance override without master's \
                     {PROVIDER_EVENT_ID_PROPERTY}"
                )
            })?;

        let instance_id = format!("{}_{}", master_id, google_instance_suffix(rid));

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
                SendUpdates::None,
                false,
                &google_event,
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to update recurring instance: {}",
                    &google_event.summary
                )
            })?;

        return Event::from_google(response.body);
    }

    // Let google change the ID
    // (Otherwise we'll get "Invalid resource id value")
    let mut google_event = cmd.event.to_google();
    google_event.id = String::new();

    let response = client
        .events()
        .insert(
            calendar_id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to create event: {}", &google_event.summary))?;

    let created_event = Event::from_google(response.body)?;

    Ok(created_event)
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
