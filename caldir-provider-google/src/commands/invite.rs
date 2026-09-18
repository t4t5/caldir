use anyhow::Result;
use caldir_core::Event;
use serde_json::Value;

use crate::google_event::ToGoogle;
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
    let body = invite_patch_body(event, account_email)?;

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
        anyhow::bail!("failed to respond to invitation: {}", error_text);
    }

    Ok(response.json().await?)
}

pub(crate) fn invite_patch_body(event: &Event, account_email: &str) -> Result<Value> {
    let attendee = event
        .find_attendee(account_email)
        .ok_or_else(|| anyhow::anyhow!("No attendee matching {account_email} on event"))?;

    let response_status = attendee
        .status
        .map(participation_status_to_google)
        .unwrap_or("needsAction");
    let reminders = event
        .to_google()
        .reminders
        .ok_or_else(|| anyhow::anyhow!("Google event conversion omitted reminders"))?;

    let body = serde_json::json!({
        "attendees": [{
            "email": attendee.email,
            "responseStatus": response_status,
            "self": true,
        }],
        "reminders": {
            "useDefault": reminders.use_default,
            "overrides": reminders.overrides,
        },
    });

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{Attendee, EventTime, Organizer, ParticipationStatus, Reminder, XProperty};
    use chrono::{TimeZone, Utc};

    use crate::constants::GOOGLE_DEFAULT_REMINDERS_PROPERTY;

    fn invite() -> Event {
        let mut event = Event::new(
            "Friday retro",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 9, 18, 14, 0, 0).unwrap()),
        );
        event.organizer = Some(Organizer::new("organizer@example.com"));
        let mut me = Attendee::new("me@example.com");
        me.status = Some(ParticipationStatus::Accepted);
        event.attendees = vec![me];
        event
    }

    #[test]
    fn invite_patch_carries_our_reminders() {
        let mut event = invite();
        event.reminders = vec![
            Reminder {
                minutes_before_start: 30,
            },
            Reminder {
                minutes_before_start: 60,
            },
        ];

        let body = invite_patch_body(&event, "me@example.com").unwrap();

        assert_eq!(
            body,
            serde_json::json!({
                "attendees": [{
                    "email": "me@example.com",
                    "responseStatus": "accepted",
                    "self": true,
                }],
                "reminders": {
                    "useDefault": false,
                    "overrides": [
                        {"method": "popup", "minutes": 30},
                        {"method": "popup", "minutes": 60},
                    ],
                },
            })
        );
    }

    #[test]
    fn invite_patch_uses_calendar_defaults_when_marker_set() {
        let mut event = invite();
        event.x_properties = vec![XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "TRUE")];

        let body = invite_patch_body(&event, "me@example.com").unwrap();

        assert_eq!(
            body["reminders"],
            serde_json::json!({"useDefault": true, "overrides": []})
        );
    }

    #[test]
    fn invite_patch_clears_reminders_without_marker() {
        let event = invite();

        let body = invite_patch_body(&event, "me@example.com").unwrap();

        assert_eq!(
            body["reminders"],
            serde_json::json!({"useDefault": false, "overrides": []})
        );
    }
}
