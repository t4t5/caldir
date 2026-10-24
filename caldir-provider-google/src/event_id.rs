use anyhow::{Result, anyhow};
use caldir_core::{Event, EventTime};

use crate::constants::GOOGLE_EVENT_ID_PROPERTY;

/// Google API id to PATCH or DELETE for `event`.
/// A recurring-instance override targets its instance, never the master:
/// an override created locally inherits the master's id.
pub(crate) fn google_api_id(event: &Event) -> Result<String> {
    match event.recurrence_id.as_ref() {
        Some(rid) => override_instance_id(event, rid.as_event_time()),
        None => event
            .x_property(GOOGLE_EVENT_ID_PROPERTY)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Event has no {GOOGLE_EVENT_ID_PROPERTY}")),
    }
}

/// Google API id of a recurring-instance override: `{master_id}_{instance suffix}`.
/// A downloaded override already carries its own instance id — use it as-is.
pub(crate) fn override_instance_id(event: &Event, rid: &EventTime) -> Result<String> {
    let provider_id = event.x_property(GOOGLE_EVENT_ID_PROPERTY).ok_or_else(|| {
        anyhow!(
            "Cannot create recurring instance override without master's \
             {GOOGLE_EVENT_ID_PROPERTY}"
        )
    })?;

    let suffix = google_instance_suffix(rid);

    if provider_id.ends_with(&format!("_{suffix}")) {
        return Ok(provider_id.to_string());
    }

    Ok(format!(
        "{}_{suffix}",
        strip_split_series_suffix(provider_id)
    ))
}

/// Editing "this and following events" splits a Google series: the new master
/// gets the id `{original_id}_R{YYYYMMDDTHHMMSS}`. Its instances drop the `_R…`
/// part (`{original_id}_{instance suffix}`), so PATCHing `{master_id}_{suffix}` 404s.
fn strip_split_series_suffix(provider_id: &str) -> &str {
    match provider_id.rsplit_once("_R") {
        Some((base, split))
            if split.len() == 15
                && split.as_bytes()[8] == b'T'
                && split
                    .bytes()
                    .enumerate()
                    .all(|(i, b)| i == 8 || b.is_ascii_digit()) =>
        {
            base
        }
        _ => provider_id,
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
    use crate::google_event::FromGoogle;
    use caldir_core::{RecurrenceId, XProperty};
    use chrono::{TimeZone, Utc};

    #[test]
    fn locally_created_override_never_targets_the_master() {
        // Update/delete must not PATCH or DELETE the whole series just because
        // the override inherited the master's X-GOOGLE-EVENT-ID.
        let rid = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 10, 23, 14, 0, 0).unwrap());
        let mut event = Event::new("Friday retro", rid.clone());
        event.recurrence_id = Some(RecurrenceId::from_event_time(rid));
        event.x_properties.push(XProperty::new(
            GOOGLE_EVENT_ID_PROPERTY,
            "master123_R20260904T140000",
        ));

        assert_eq!(google_api_id(&event).unwrap(), "master123_20261023T140000Z");
    }

    #[test]
    fn non_override_uses_its_own_google_id() {
        let start = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 10, 23, 14, 0, 0).unwrap());
        let mut event = Event::new("Lunch", start);
        event
            .x_properties
            .push(XProperty::new(GOOGLE_EVENT_ID_PROPERTY, "abc123"));

        assert_eq!(google_api_id(&event).unwrap(), "abc123");
    }

    #[test]
    fn synthesized_override_gets_the_suffix_appended_to_the_master_id() {
        // An override created locally inherits the MASTER's X-GOOGLE-EVENT-ID.
        let rid = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 8, 14, 17, 0, 0).unwrap());
        let mut event = Event::new("Weekly sync", rid.clone());
        event.recurrence_id = Some(RecurrenceId::from_event_time(rid.clone()));
        event
            .x_properties
            .push(XProperty::new(GOOGLE_EVENT_ID_PROPERTY, "master123"));

        assert_eq!(
            override_instance_id(&event, &rid).unwrap(),
            "master123_20260814T170000Z"
        );
    }

    #[test]
    fn override_of_a_split_series_targets_the_original_series_instance_id() {
        // After "this and following" edits, the new master's id carries an
        // `_R{split start}` suffix, but Google's instance ids do not:
        // instances of "master123_R20260904T140000" are "master123_{suffix}".
        let rid = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 10, 23, 14, 0, 0).unwrap());
        let mut event = Event::new("Friday retro", rid.clone());
        event.recurrence_id = Some(RecurrenceId::from_event_time(rid.clone()));
        event.x_properties.push(XProperty::new(
            GOOGLE_EVENT_ID_PROPERTY,
            "master123_R20260904T140000",
        ));

        assert_eq!(
            override_instance_id(&event, &rid).unwrap(),
            "master123_20261023T140000Z"
        );
    }

    #[test]
    fn ids_that_merely_contain_r_are_not_treated_as_split_series() {
        assert_eq!(strip_split_series_suffix("abc_Rdef"), "abc_Rdef");
        assert_eq!(strip_split_series_suffix("abc_R2026"), "abc_R2026");
        assert_eq!(strip_split_series_suffix("master123"), "master123");
    }

    #[test]
    fn recreating_a_downloaded_override_must_not_double_its_google_id() {
        // When the organizer moves one occurrence of a recurring event,
        // Google stores the modified occurrence under its own event id:
        // "{master_id}_{original start as UTC}". Here the 17:00 occurrence
        // of "master123" was moved to 18:00:
        let downloaded: google_calendar::types::Event = serde_json::from_value(serde_json::json!({
            "id": "master123_20260814T170000Z",
            "iCalUID": "master123@google.com",
            "summary": "Weekly sync",
            "status": "confirmed",
            "recurringEventId": "master123",
            "originalStartTime": { "dateTime": "2026-08-14T17:00:00Z" },
            "start": { "dateTime": "2026-08-14T18:00:00Z" },
            "end": { "dateTime": "2026-08-14T19:00:00Z" },
        }))
        .unwrap();

        // Pulling it stores that id — suffix included — in X-GOOGLE-EVENT-ID:
        let event = Event::from_google(downloaded).unwrap();
        assert_eq!(
            event.x_property(GOOGLE_EVENT_ID_PROPERTY).unwrap(),
            "master123_20260814T170000Z"
        );

        // If sync state is lost, the local file is mistaken for a new event
        // and pushed through create_event (rencal#99). The PATCH must target
        // the id Google already knows — a doubled suffix 404s:
        let rid = event.recurrence_id.clone().unwrap();
        assert_eq!(
            override_instance_id(&event, rid.as_event_time()).unwrap(),
            "master123_20260814T170000Z"
        );
    }
}
