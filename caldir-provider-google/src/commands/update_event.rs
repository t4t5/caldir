use anyhow::{Result, anyhow, bail};
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::UpdateEvent;
use serde_json::Value;

use crate::app_config::AppConfigStore;
use crate::commands::invite::patch_invite_status;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;

    // Get Google's event ID from custom properties
    let google_event_id = cmd
        .event
        .x_property(PROVIDER_EVENT_ID_PROPERTY)
        .ok_or_else(|| anyhow!("Cannot update event without {PROVIDER_EVENT_ID_PROPERTY}"))?;

    if cmd.event.is_invite_for(account_email) {
        // Only update our own attendee status:
        let google_event = patch_invite_status(
            &session,
            calendar_id,
            google_event_id,
            &cmd.event,
            account_email,
        )
        .await?;

        Ok(Event::from_google(google_event)?)
    } else {
        // Organizer or own event: PATCH event fields, but never send attendees.
        // A single EXDATE edit is a master update; sending a full attendee list
        // here can rewrite invite state across the whole series.
        let google_event = patch_event_without_attendees(
            session.access_token(),
            calendar_id,
            google_event_id,
            &cmd.event,
        )
        .await?;

        Ok(Event::from_google(google_event)?)
    }
}

async fn patch_event_without_attendees(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
    event: &Event,
) -> Result<google_calendar::types::Event> {
    let body = patch_body_without_attendees(event)?;

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}?\
         sendUpdates=all&conferenceDataVersion=1",
        calendar_id, event_id,
    );

    let response = reqwest::Client::new()
        .patch(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        bail!("Error handling request: {}", error_text);
    }

    Ok(response.json().await?)
}

fn patch_body_without_attendees(event: &Event) -> Result<Value> {
    let mut body = serde_json::to_value(event.to_google())?;

    if let Value::Object(fields) = &mut body {
        fields.remove("attendees");
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{Attendee, EventTime, Recurrence};
    use chrono::NaiveDate;

    #[test]
    fn patch_body_omits_attendees() {
        let mut event = Event::new(
            "Weekly sync",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
        );
        event.recurrence = Some(Recurrence::new("FREQ=WEEKLY"));
        event.attendees = vec![Attendee::new("alice@example.com")];

        let body = patch_body_without_attendees(&event).unwrap();

        assert!(body.get("attendees").is_none());
        assert_eq!(
            body.get("summary").and_then(Value::as_str),
            Some("Weekly sync")
        );
        assert!(body.get("recurrence").is_some());
    }
}
