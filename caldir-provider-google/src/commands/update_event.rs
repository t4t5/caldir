use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::UpdateEvent;
use google_calendar::types::SendUpdates;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::google_event::to_google::participation_status_to_google;
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let session = Session::load_valid(account_email).await?;

    // Get Google's event ID from custom properties
    let google_event_id = cmd
        .event
        .custom_properties
        .iter()
        .find(|(k, _)| k == PROVIDER_EVENT_ID_PROPERTY)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot update event without {PROVIDER_EVENT_ID_PROPERTY}")
        })?;

    if cmd.event.is_invite_for(account_email) {
        // Only update our own attendee status:
        let google_event = update_invite_status(
            &session,
            calendar_id,
            google_event_id,
            &cmd.event,
            account_email,
        )
        .await?;
        Ok(Event::from_google(google_event)?)
    } else {
        // Organizer or own event: full PUT update
        let client = session.client()?;
        let google_event = cmd.event.to_google();

        let response = client
            .events()
            .update(
                calendar_id,
                google_event_id,
                0,
                0,
                false,
                SendUpdates::None,
                false,
                &google_event,
            )
            .await?;

        Ok(Event::from_google(response.body)?)
    }
}

/// Non-organizer: only PATCH our own attendee response status.
/// Uses a raw HTTP PATCH with a minimal JSON body to avoid sending
/// shared properties (like guestsCanInviteOthers) that the google_calendar
/// crate's Event struct serializes by default, which triggers 403 from Google.
async fn update_invite_status(
    session: &Session,
    calendar_id: &str,
    event_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<google_calendar::types::Event> {
    let attendee = event.find_attendee(account_email).unwrap();
    let response_status = attendee
        .response_status
        .map(participation_status_to_google)
        .unwrap_or("needsAction");

    let body = serde_json::json!({
        "attendees": [{
            "email": attendee.email,
            "responseStatus": response_status,
            "self": true,
        }]
    });

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}",
        calendar_id, event_id,
    );

    let response = reqwest::Client::new()
        .patch(&url)
        .bearer_auth(session.access_token())
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Error handling request: {}", error_text);
    }

    Ok(response.json().await?)
}
