use std::collections::HashMap;

use anyhow::{Context, Result};
use caldir_core::event::{CustomProperty, Event, EventStatus, EventTime, Reminders};
use caldir_core::remote::protocol::ListEvents;
use google_calendar::types::OrderBy;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::google_event::{FromGoogle, google_dt_to_event_time};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    let google_events = client
        .events()
        .list_all(
            calendar_id,
            "",
            0,
            OrderBy::default(),
            &[],
            "", // search query
            &[],
            false,
            false,
            false,
            &cmd.to,
            &cmd.from,
            "",
            "",
        )
        .await
        .context("Failed to fetch events")?
        .body;

    process_google_events(google_events)
}

/// Convert Google's raw events list into caldir Events.
///
/// Google returns cancelled instances of recurring events as bare tombstones:
/// status="cancelled" with no start/end, but with recurringEventId +
/// originalStartTime. We split these out and convert each into a cancelled
/// instance override Event so the recurrence engine can skip the corresponding
/// occurrence.
///
/// Standalone deleted events arrive as tombstones too (no recurringEventId);
/// those are dropped — there's nothing local to attach them to.
fn process_google_events(google_events: Vec<google_calendar::types::Event>) -> Result<Vec<Event>> {
    let mut master_info_by_google_id: HashMap<String, (String, String)> = HashMap::new();
    let mut cancellations: Vec<google_calendar::types::Event> = Vec::new();
    let mut events: Vec<Event> = Vec::with_capacity(google_events.len());

    for ge in google_events {
        let has_start = ge
            .start
            .as_ref()
            .is_some_and(|s| s.date_time.is_some() || s.date.is_some());

        if !has_start && ge.status == "cancelled" {
            if !ge.recurring_event_id.is_empty() {
                cancellations.push(ge);
            }
            continue;
        }

        if !ge.i_cal_uid.is_empty() {
            master_info_by_google_id
                .insert(ge.id.clone(), (ge.i_cal_uid.clone(), ge.summary.clone()));
        }
        events.push(Event::from_google(ge)?);
    }

    for ge in cancellations {
        if let Some(event) = cancellation_to_event(&ge, &master_info_by_google_id) {
            events.push(event);
        }
    }

    Ok(events)
}

fn cancellation_to_event(
    ge: &google_calendar::types::Event,
    master_info_by_google_id: &HashMap<String, (String, String)>,
) -> Option<Event> {
    let recurrence_id = google_dt_to_event_time(ge.original_start_time.as_ref())?;
    let (uid, summary) = master_info_by_google_id
        .get(&ge.recurring_event_id)
        .cloned()
        .unwrap_or_else(|| {
            (
                format!("{}@google.com", ge.recurring_event_id),
                String::new(),
            )
        });

    let start = recurrence_id.clone();
    let end = match &start {
        EventTime::Date(d) => EventTime::Date(*d),
        EventTime::DateTimeUtc(dt) => EventTime::DateTimeUtc(*dt),
        EventTime::DateTimeFloating(dt) => EventTime::DateTimeFloating(*dt),
        EventTime::DateTimeZoned { datetime, tzid } => EventTime::DateTimeZoned {
            datetime: *datetime,
            tzid: tzid.clone(),
        },
    };

    Some(Event {
        uid,
        summary,
        description: None,
        location: None,
        start,
        end,
        status: EventStatus::Cancelled,
        recurrence: None,
        recurrence_id: Some(recurrence_id),
        reminders: Reminders(Vec::new()),
        transparency: caldir_core::event::Transparency::Opaque,
        organizer: None,
        attendees: Vec::new(),
        conference_url: None,
        updated: ge.updated,
        sequence: if ge.sequence > 0 {
            Some(ge.sequence)
        } else {
            None
        },
        custom_properties: vec![CustomProperty::new(PROVIDER_EVENT_ID_PROPERTY, &ge.id)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_calendar::types as g;

    fn empty_event() -> g::Event {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn empty_dt() -> g::EventDateTime {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn master(id: &str, uid: &str, summary: &str) -> g::Event {
        g::Event {
            id: id.into(),
            i_cal_uid: uid.into(),
            summary: summary.into(),
            status: "confirmed".into(),
            start: Some(g::EventDateTime {
                date_time: Some("2026-01-16T15:00:00Z".parse().unwrap()),
                time_zone: "Europe/Oslo".into(),
                ..empty_dt()
            }),
            end: Some(g::EventDateTime {
                date_time: Some("2026-01-16T15:45:00Z".parse().unwrap()),
                time_zone: "Europe/Oslo".into(),
                ..empty_dt()
            }),
            recurrence: vec!["RRULE:FREQ=WEEKLY;BYDAY=FR".into()],
            ..empty_event()
        }
    }

    fn cancelled_instance(id: &str, master_id: &str, original_start: &str) -> g::Event {
        g::Event {
            id: id.into(),
            status: "cancelled".into(),
            recurring_event_id: master_id.into(),
            original_start_time: Some(g::EventDateTime {
                date_time: Some(original_start.parse().unwrap()),
                time_zone: "Europe/Oslo".into(),
                ..empty_dt()
            }),
            ..empty_event()
        }
    }

    #[test]
    fn cancelled_recurring_instance_becomes_cancelled_override() {
        let result = process_google_events(vec![
            master("master_id", "uid@google.com", "Weekly retro"),
            cancelled_instance(
                "master_id_20260213T150000Z",
                "master_id",
                "2026-02-13T15:00:00Z",
            ),
        ])
        .unwrap();

        assert_eq!(result.len(), 2);
        let cancellation = result.iter().find(|e| e.recurrence_id.is_some()).unwrap();
        assert_eq!(cancellation.uid, "uid@google.com");
        assert_eq!(cancellation.summary, "Weekly retro");
        assert_eq!(cancellation.status, EventStatus::Cancelled);
        assert!(matches!(
            cancellation.recurrence_id,
            Some(EventTime::DateTimeZoned { ref tzid, .. }) if tzid == "Europe/Oslo"
        ));
    }

    #[test]
    fn cancellation_without_master_in_batch_falls_back_to_synthetic_uid() {
        // The master may sit outside the requested date range — we still want the
        // cancellation captured so the engine can skip the occurrence.
        let result = process_google_events(vec![cancelled_instance(
            "orphan_id_20260213T150000Z",
            "orphan_master",
            "2026-02-13T15:00:00Z",
        )])
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uid, "orphan_master@google.com");
        assert_eq!(result[0].summary, "");
        assert_eq!(result[0].status, EventStatus::Cancelled);
    }

    #[test]
    fn standalone_cancelled_tombstone_is_dropped() {
        // Status=cancelled, no recurringEventId — a deleted standalone event.
        // Nothing to attach it to locally; drop silently.
        let mut tombstone = empty_event();
        tombstone.id = "deleted_id".into();
        tombstone.status = "cancelled".into();

        let result = process_google_events(vec![tombstone]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn cancellation_without_original_start_is_dropped() {
        // We can't attach it to a specific occurrence without originalStartTime.
        let mut bad = empty_event();
        bad.id = "x".into();
        bad.status = "cancelled".into();
        bad.recurring_event_id = "master".into();

        let result = process_google_events(vec![bad]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn normal_events_pass_through() {
        let result =
            process_google_events(vec![master("m1", "uid1@google.com", "Weekly retro")]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uid, "uid1@google.com");
        assert!(result[0].recurrence.is_some());
        assert_eq!(result[0].status, EventStatus::Confirmed);
    }
}
