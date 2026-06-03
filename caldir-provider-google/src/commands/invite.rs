use anyhow::Result;
use caldir_core::Event;

use crate::google_event::to_google::participation_status_to_google;
use crate::session::Session;

/// RSVP to event we're invited to.
/// PATCH only our own attendee `responseStatus`
pub(crate) async fn patch_invite_status(
    session: &Session,
    calendar_id: &str,
    event_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<google_calendar::types::Event> {
    let attendee = event
        .find_attendee(account_email)
        .ok_or_else(|| anyhow::anyhow!("No attendee matching {account_email} on event"))?;

    let response_status = attendee
        .status
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
