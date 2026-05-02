use anyhow::{Context, Result, anyhow};
use caldir_core::event::{Event, EventTime};
use caldir_core::remote::protocol::CreateEvent;
use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::graph_api::client::GraphClient;
use crate::graph_api::types::{GraphEvent, GraphResponse};
use crate::outlook_event::from_outlook::from_outlook;
use crate::outlook_event::to_outlook::to_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    // Recurring instance overrides share the master's iCalUId, so POSTing one
    // to /events would create an unrelated standalone event (Graph has no
    // "create exception" affordance on that endpoint). Look up the
    // auto-expanded instance and PATCH it instead — Graph attaches the
    // override to the series and assigns it a fresh per-instance id.
    if let Some(rid) = cmd.event.recurrence_id.as_ref() {
        let master_id = cmd
            .event
            .custom_properties
            .iter()
            .find(|(k, _)| k == PROVIDER_EVENT_ID_PROPERTY)
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "Cannot create recurring instance override without master's \
                     {PROVIDER_EVENT_ID_PROPERTY}"
                )
            })?;

        let instance_id = find_instance_id(&graph, master_id, rid).await?;

        let body = to_outlook(&cmd.event);
        let path = format!("/me/events/{}", instance_id);
        let response = graph.patch(&path, &body).await.with_context(|| {
            format!("Failed to patch recurring instance: {}", cmd.event.summary)
        })?;

        let updated: GraphEvent = response
            .json()
            .await
            .context("Failed to parse patched instance response")?;

        let mut event = from_outlook(updated, &config.outlook_account)?;
        // Graph's PATCH-instance response carries the exception's per-instance
        // iCalUId and omits originalStart, so from_outlook would produce a
        // standalone-looking event. Restore the master's UID and the sent
        // recurrence_id so the override file has a stable (uid, recurrence_id)
        // key matching what list_events emits on subsequent pulls.
        event.uid = cmd.event.uid.clone();
        event.recurrence_id = cmd.event.recurrence_id.clone();
        return Ok(event);
    }

    let body = to_outlook(&cmd.event);

    let path = format!("/me/calendars/{}/events", config.outlook_calendar_id);
    let response = graph
        .post(&path, &body)
        .await
        .with_context(|| format!("Failed to create event: {}", cmd.event.summary))?;

    let created: GraphEvent = response
        .json()
        .await
        .context("Failed to parse created event response")?;

    from_outlook(created, &config.outlook_account)
}

/// Find the Outlook instance id whose `originalStart` matches the override's
/// `recurrence_id`. We query a ±1 day window around the recurrence_id so
/// timezone skew doesn't push the instance out of range.
async fn find_instance_id(
    graph: &GraphClient,
    master_id: &str,
    recurrence_id: &EventTime,
) -> Result<String> {
    let rid_utc = recurrence_id.to_utc().ok_or_else(|| {
        anyhow!(
            "Cannot resolve recurrence_id {} to a UTC instant",
            recurrence_id.to_iso_string()
        )
    })?;

    let start = (rid_utc - Duration::days(1)).format("%Y-%m-%dT%H:%M:%SZ");
    let end = (rid_utc + Duration::days(1)).format("%Y-%m-%dT%H:%M:%SZ");

    let path = format!(
        "/me/events/{}/instances?startDateTime={}&endDateTime={}",
        master_id, start, end
    );
    let response = graph
        .get(&path)
        .await
        .with_context(|| format!("Failed to list instances of master {}", master_id))?;

    let body: GraphResponse<GraphEvent> = response
        .json()
        .await
        .context("Failed to parse instances response")?;

    match find_matching_instance(&body.value, recurrence_id) {
        Some(instance) => Ok(instance.id.clone()),
        None => Err(anyhow!(
            "No Outlook instance found for recurrence_id={} on master={}",
            recurrence_id.to_iso_string(),
            master_id
        )),
    }
}

/// Pick the instance whose scheduled start lines up with `recurrence_id`.
/// Prefers `originalStart` (RFC3339 UTC) when present and falls back to the
/// instance's `start` for parity with non-modified occurrences.
fn find_matching_instance<'a>(
    instances: &'a [GraphEvent],
    recurrence_id: &EventTime,
) -> Option<&'a GraphEvent> {
    let target = recurrence_id.to_utc()?;
    instances
        .iter()
        .find(|inst| instance_original_start_utc(inst) == Some(target))
}

fn instance_original_start_utc(instance: &GraphEvent) -> Option<DateTime<Utc>> {
    if let Some(s) = instance.original_start.as_deref()
        && let Some(dt) = parse_original_start_utc(s, instance.is_all_day)
    {
        return Some(dt);
    }
    let dtz = instance.start.as_ref()?;
    parse_start_utc(&dtz.date_time, instance.is_all_day)
}

fn parse_original_start_utc(s: &str, is_all_day: bool) -> Option<DateTime<Utc>> {
    if is_all_day {
        let date = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").ok()?;
        return date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_start_utc(s: &str, is_all_day: bool) -> Option<DateTime<Utc>> {
    if is_all_day {
        let date = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").ok()?;
        return date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .map(|dt| dt.and_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn instance(id: &str, original_start: Option<&str>) -> GraphEvent {
        GraphEvent {
            id: id.to_string(),
            i_cal_uid: String::new(),
            subject: String::new(),
            body: None,
            start: None,
            end: None,
            location: None,
            is_all_day: false,
            is_cancelled: false,
            recurrence: None,
            attendees: Vec::new(),
            organizer: None,
            reminder_minutes_before_start: 0,
            is_reminder_on: false,
            show_as: String::new(),
            last_modified_date_time: None,
            online_meeting: None,
            original_start: original_start.map(|s| s.to_string()),
            response_status: None,
            event_type: String::new(),
        }
    }

    #[test]
    fn matches_by_original_start_utc() {
        let instances = vec![
            instance("inst-1", Some("2026-05-06T16:00:00Z")),
            instance("inst-2", Some("2026-05-07T16:00:00Z")),
            instance("inst-3", Some("2026-05-08T16:00:00Z")),
        ];

        let rid = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 7, 16, 0, 0).unwrap());

        let matched = find_matching_instance(&instances, &rid).expect("should find match");
        assert_eq!(matched.id, "inst-2");
    }

    #[test]
    fn returns_none_when_no_instance_matches() {
        let instances = vec![
            instance("inst-1", Some("2026-05-06T16:00:00Z")),
            instance("inst-3", Some("2026-05-08T16:00:00Z")),
        ];

        let rid = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 7, 16, 0, 0).unwrap());

        assert!(find_matching_instance(&instances, &rid).is_none());
    }

    #[test]
    fn matches_zoned_recurrence_id_to_utc_original_start() {
        // 18:00 Europe/Stockholm in May (DST active, UTC+2) == 16:00 UTC.
        let instances = vec![instance("inst-zoned", Some("2026-05-07T16:00:00Z"))];

        let rid = EventTime::DateTimeZoned {
            datetime: chrono::NaiveDate::from_ymd_opt(2026, 5, 7)
                .unwrap()
                .and_hms_opt(18, 0, 0)
                .unwrap(),
            tzid: "Europe/Stockholm".to_string(),
        };

        let matched = find_matching_instance(&instances, &rid).expect("should match");
        assert_eq!(matched.id, "inst-zoned");
    }
}
