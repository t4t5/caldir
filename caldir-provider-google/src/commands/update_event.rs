use anyhow::{Result, anyhow, bail};
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::UpdateEvent;
use serde_json::Value;

use crate::app_config::AppConfigStore;
use crate::commands::invite::patch_invite_status;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_EVENT_TYPE_PROPERTY, PROVIDER_NAME};
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

const BIRTHDAY_PATCH_FIELDS: &[&str] = &["summary", "colorId", "reminders"];

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

pub(crate) async fn patch_event_without_attendees(
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
        if event.x_property(PROVIDER_EVENT_TYPE_PROPERTY) == Some("birthday") {
            fields.retain(|key, _| BIRTHDAY_PATCH_FIELDS.contains(&key.as_str()));

            return Ok(body);
        }

        fields.remove("attendees");

        for key in ["start", "end"] {
            let Some(Value::Object(time)) = fields.get_mut(key) else {
                continue;
            };

            if time.contains_key("dateTime") {
                time.insert("date".to_string(), Value::Null);
            } else if time.contains_key("date") {
                time.insert("dateTime".to_string(), Value::Null);
                time.insert("timeZone".to_string(), Value::Null);
            }
        }
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{Attendee, EventTime, Recurrence, Reminder, XProperty};
    use chrono::{NaiveDate, TimeZone, Utc};

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
        assert!(body.get("conferenceData").is_none());
        assert_eq!(
            body.get("summary").and_then(Value::as_str),
            Some("Weekly sync")
        );
        assert!(body.get("recurrence").is_some());
    }

    #[test]
    fn birthday_patch_only_updates_supported_fields() {
        let mut event = Event::new(
            "Alice's birthday",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
        );
        event.description = Some("Should not be sent".into());
        event.location = Some("This should not be sent either".into());
        event.reminders = vec![Reminder {
            minutes_before_start: 15,
        }];
        event.x_properties = vec![
            XProperty::new(PROVIDER_EVENT_TYPE_PROPERTY, "birthday"),
            XProperty::new(crate::constants::PROVIDER_COLOR_ID_PROPERTY, "5"),
        ];

        let body = patch_body_without_attendees(&event).unwrap();

        assert_eq!(
            body,
            serde_json::json!({
                "summary": "Alice's birthday",
                "colorId": "5",
                "reminders": {
                    "overrides": [{"method": "popup", "minutes": 15}],
                    "useDefault": false,
                },
            })
        );
    }

    #[test]
    fn patch_body_includes_description() {
        let mut event = Event::new(
            "Planning",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
        );
        event.description = Some("Bring the roadmap".into());

        let body = patch_body_without_attendees(&event).unwrap();

        assert_eq!(
            body.get("description").and_then(Value::as_str),
            Some("Bring the roadmap")
        );
    }

    #[test]
    fn timed_patch_explicitly_clears_all_day_date() {
        let mut event = Event::new(
            "Timed event",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap()),
        );
        event.end = Some(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 7, 10, 10, 0, 0).unwrap(),
        ));

        let body = patch_body_without_attendees(&event).unwrap();

        for key in ["start", "end"] {
            let time = body.get(key).and_then(Value::as_object).unwrap();
            assert!(time.get("dateTime").is_some());
            assert_eq!(time.get("date"), Some(&Value::Null));
        }
    }

    #[test]
    fn all_day_patch_explicitly_clears_timed_fields() {
        let mut event = Event::new(
            "All-day event",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
        );
        event.end = Some(EventTime::Date(
            NaiveDate::from_ymd_opt(2026, 7, 11).unwrap(),
        ));

        let body = patch_body_without_attendees(&event).unwrap();

        for key in ["start", "end"] {
            let time = body.get(key).and_then(Value::as_object).unwrap();
            assert!(time.get("date").is_some_and(|date| !date.is_null()));
            assert_eq!(time.get("dateTime"), Some(&Value::Null));
            assert_eq!(time.get("timeZone"), Some(&Value::Null));
        }
    }
}
