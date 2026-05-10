mod error;
mod event_uid;
mod recurrence_id;

use crate::EventTime;
use chrono::{NaiveDate, NaiveDateTime};
pub use error::EventInstanceIdError;
pub use event_uid::EventUid;
pub use recurrence_id::RecurrenceId;

const RID_SEPARATOR: &str = "__";
const TZID_PREFIX: &str = "TZID=";

// UID + RecurrenceId = the actual unique ID per event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventInstanceId((EventUid, Option<RecurrenceId>));

impl EventInstanceId {
    pub fn new(uid: EventUid, recurrence_id: Option<RecurrenceId>) -> Self {
        EventInstanceId((uid, recurrence_id))
    }

    pub fn uid(&self) -> &EventUid {
        &self.0.0
    }

    pub fn recurrence_id(&self) -> Option<&RecurrenceId> {
        self.0.1.as_ref()
    }

    pub fn from_str(s: &str) -> Result<Self, EventInstanceIdError> {
        match s.rsplit_once(RID_SEPARATOR) {
            Some((uid_str, rid_str)) => {
                let event_time = parse_recurrence_id(rid_str)?;

                Ok(EventInstanceId::new(
                    EventUid::from_str(uid_str.to_string()),
                    Some(RecurrenceId::from_event_time(event_time)),
                ))
            }
            None => Ok(EventInstanceId::new(
                EventUid::from_str(s.to_string()),
                None,
            )),
        }
    }

    // Example: abc123@google.com__20260101T170000Z
    pub fn to_str(&self) -> String {
        let uid_str = self.uid().as_str();
        match self.recurrence_id() {
            Some(rid) => format!(
                "{}{}{}",
                uid_str,
                RID_SEPARATOR,
                format_recurrence_id(rid.as_event_time())
            ),
            None => uid_str.to_string(),
        }
    }
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

fn parse_recurrence_id(s: &str) -> Result<EventTime, EventInstanceIdError> {
    let invalid = || EventInstanceIdError::InvalidRecurrenceId(s.to_string());

    if let Some(rest) = s.strip_prefix(TZID_PREFIX) {
        let (tzid, dt_str) = rest.split_once(':').ok_or_else(invalid)?;

        let datetime =
            NaiveDateTime::parse_from_str(dt_str, "%Y%m%dT%H%M%S").map_err(|_| invalid())?;

        return Ok(EventTime::DateTimeZoned {
            datetime,
            tzid: tzid.to_string(),
        });
    }

    if s.ends_with('Z') {
        let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ").map_err(|_| invalid())?;

        return Ok(EventTime::DateTimeUtc(datetime.and_utc()));
    }

    if !s.contains('T') {
        let date = NaiveDate::parse_from_str(s, "%Y%m%d").map_err(|_| invalid())?;
        return Ok(EventTime::Date(date));
    }

    let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").map_err(|_| invalid())?;

    Ok(EventTime::DateTimeFloating(datetime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    fn uid() -> EventUid {
        EventUid::from_str("abc123@hooli.com".to_string())
    }

    fn assert_round_trip(id: &EventInstanceId, expected_str: &str) {
        let s = id.to_str();
        assert_eq!(s, expected_str);
        let parsed = EventInstanceId::from_str(&s).unwrap();
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
    fn round_trips_uid_containing_double_underscore() {
        // Some providers emit UIDs with embedded "__" — rsplit_once on the
        // last occurrence keeps the rid intact.
        let weird_uid = EventUid::from_str("event__weird@example.com".to_string());
        let id = EventInstanceId::new(
            weird_uid,
            Some(RecurrenceId::from_event_time(EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(),
            ))),
        );
        assert_round_trip(&id, "event__weird@example.com__20260109");
    }

    #[test]
    fn rejects_malformed_recurrence_id() {
        let result = EventInstanceId::from_str("abc__not-a-date");
        assert_eq!(
            result,
            Err(EventInstanceIdError::InvalidRecurrenceId(
                "not-a-date".to_string()
            ))
        );
    }

    #[test]
    fn rejects_zoned_without_datetime() {
        let result = EventInstanceId::from_str("abc__TZID=Europe/Stockholm");
        assert!(matches!(
            result,
            Err(EventInstanceIdError::InvalidRecurrenceId(_))
        ));
    }
}
