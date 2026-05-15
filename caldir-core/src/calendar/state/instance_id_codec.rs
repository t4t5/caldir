//! String codec for `EventInstanceId` used by the `known_event_ids` file.
//!
//! Format: `{uid}` for non-recurring events, `{uid}__{recurrence_id}` for
//! instance overrides. Recurrence id sub-formats:
//!   - `YYYYMMDD`                       — all-day
//!   - `YYYYMMDDTHHMMSSZ`                — UTC
//!   - `YYYYMMDDTHHMMSS`                 — floating
//!   - `TZID={tzid}:YYYYMMDDTHHMMSS`     — zoned
//!
//! UIDs may legitimately contain `__` (RFC 5545 allows any text), so decode
//! falls back to treating the whole string as the UID when the suffix after
//! the last `__` isn't a parseable recurrence id.

use crate::EventTime;
use crate::event::{EventInstanceId, EventUid, RecurrenceId};
use chrono::{NaiveDate, NaiveDateTime};

const RID_SEPARATOR: &str = "__";
const TZID_PREFIX: &str = "TZID=";

pub(super) fn encode(id: &EventInstanceId) -> String {
    let uid_str = id.uid().as_str();
    match id.recurrence_id() {
        Some(rid) => format!(
            "{}{}{}",
            uid_str,
            RID_SEPARATOR,
            format_recurrence_id(rid.as_event_time())
        ),
        None => uid_str.to_string(),
    }
}

pub(super) fn decode(s: &str) -> EventInstanceId {
    if let Some((uid_str, rid_str)) = s.rsplit_once(RID_SEPARATOR)
        && let Some(event_time) = parse_recurrence_id(rid_str)
    {
        return EventInstanceId::new(
            EventUid::new(uid_str.to_string()),
            Some(RecurrenceId::from_event_time(event_time)),
        );
    }

    EventInstanceId::new(EventUid::new(s.to_string()), None)
}

fn format_recurrence_id(event_time: &EventTime) -> String {
    match event_time {
        EventTime::Date(date) => date.format("%Y%m%d").to_string(),
        EventTime::DateTimeUtc(datetime) => datetime.format("%Y%m%dT%H%M%SZ").to_string(),
        EventTime::DateTimeFloating(datetime) => datetime.format("%Y%m%dT%H%M%S").to_string(),
        EventTime::DateTimeZoned { datetime, tzid } => {
            format!("{TZID_PREFIX}{tzid}:{}", datetime.format("%Y%m%dT%H%M%S"))
        }
    }
}

fn parse_recurrence_id(s: &str) -> Option<EventTime> {
    if let Some(rest) = s.strip_prefix(TZID_PREFIX) {
        let (tzid, dt_str) = rest.split_once(':')?;
        let datetime = NaiveDateTime::parse_from_str(dt_str, "%Y%m%dT%H%M%S").ok()?;

        return Some(EventTime::DateTimeZoned {
            datetime,
            tzid: tzid.to_string(),
        });
    }

    if s.ends_with('Z') {
        let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ").ok()?;
        return Some(EventTime::DateTimeUtc(datetime.and_utc()));
    }

    if !s.contains('T') {
        let date = NaiveDate::parse_from_str(s, "%Y%m%d").ok()?;
        return Some(EventTime::Date(date));
    }

    let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?;
    Some(EventTime::DateTimeFloating(datetime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    fn uid() -> EventUid {
        EventUid::new("abc123@hooli.com".to_string())
    }

    fn assert_round_trip(id: &EventInstanceId, expected_str: &str) {
        let s = encode(id);
        assert_eq!(s, expected_str);
        let parsed = decode(&s);
        assert_eq!(&parsed, id);
    }

    #[test]
    fn round_trips_non_recurring_event() {
        let id = EventInstanceId::new(uid(), None);
        assert_round_trip(&id, "abc123@hooli.com");
    }

    #[test]
    fn round_trips_date_recurrence_id() {
        let id = EventInstanceId::new(
            uid(),
            Some(RecurrenceId::from_event_time(EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(),
            ))),
        );
        assert_round_trip(&id, "abc123@hooli.com__20260109");
    }

    #[test]
    fn round_trips_utc_recurrence_id() {
        let id = EventInstanceId::new(
            uid(),
            Some(RecurrenceId::from_event_time(EventTime::DateTimeUtc(
                chrono::Utc.with_ymd_and_hms(2026, 1, 1, 17, 0, 0).unwrap(),
            ))),
        );
        assert_round_trip(&id, "abc123@hooli.com__20260101T170000Z");
    }

    #[test]
    fn round_trips_floating_recurrence_id() {
        let datetime = NaiveDate::from_ymd_opt(2026, 1, 9)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let id = EventInstanceId::new(
            uid(),
            Some(RecurrenceId::from_event_time(EventTime::DateTimeFloating(
                datetime,
            ))),
        );
        assert_round_trip(&id, "abc123@hooli.com__20260109T100000");
    }

    #[test]
    fn round_trips_zoned_recurrence_id() {
        let datetime = NaiveDate::from_ymd_opt(2026, 1, 9)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let id = EventInstanceId::new(
            uid(),
            Some(RecurrenceId::from_event_time(EventTime::DateTimeZoned {
                datetime,
                tzid: "Europe/Stockholm".to_string(),
            })),
        );
        assert_round_trip(
            &id,
            "abc123@hooli.com__TZID=Europe/Stockholm:20260109T100000",
        );
    }

    #[test]
    fn round_trips_uid_containing_double_underscore_with_recurrence_id() {
        // Some providers emit UIDs with embedded "__" — rsplit_once on the
        // last occurrence keeps the rid intact.
        let weird_uid = EventUid::new("event__weird@example.com".to_string());
        let id = EventInstanceId::new(
            weird_uid,
            Some(RecurrenceId::from_event_time(EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(),
            ))),
        );
        assert_round_trip(&id, "event__weird@example.com__20260109");
    }

    #[test]
    fn round_trips_uid_containing_double_underscore_without_recurrence_id() {
        // Without a trailing rid, the suffix after the last "__" isn't a
        // valid recurrence id, so the whole string is the UID.
        let id = EventInstanceId::new(EventUid::new("event__weird@example.com".to_string()), None);
        assert_round_trip(&id, "event__weird@example.com");
    }

    #[test]
    fn treats_malformed_recurrence_suffix_as_part_of_uid() {
        let id = decode("abc__not-a-date");
        assert_eq!(id.uid().as_str(), "abc__not-a-date");
        assert!(id.recurrence_id().is_none());
    }

    #[test]
    fn treats_zoned_suffix_without_datetime_as_part_of_uid() {
        let id = decode("abc__TZID=Europe/Stockholm");
        assert_eq!(id.uid().as_str(), "abc__TZID=Europe/Stockholm");
        assert!(id.recurrence_id().is_none());
    }
}
