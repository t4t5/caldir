mod attendee;
mod error;
mod from_icalendar;
mod instance_id;
mod organizer;
mod recurrence;
mod reminder;
mod slugify;
mod status;
mod time;
mod to_icalendar;
mod transparency;
mod x_property;

use attendee::Attendee;
use chrono::{DateTime, Utc};
pub use error::EventError;
pub use instance_id::{EventInstanceId, EventInstanceIdError, EventUid, RecurrenceId};
pub use organizer::Organizer;
pub use recurrence::Recurrence;
pub use reminder::Reminder;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub use status::Status;
pub use time::EventTime;
pub use transparency::Transparency;
pub use x_property::XProperty;

const ICS_PRODID: &str = "CALDIR";
const ICS_VERSION: &str = "2.0";
const ICS_UID_DOMAIN: &str = "caldir";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub uid: EventUid,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventTime,
    pub end: Option<EventTime>,
    pub status: Option<Status>,
    pub transparency: Option<Transparency>,
    pub recurrence: Option<Recurrence>,
    pub recurrence_id: Option<RecurrenceId>,
    pub last_modified: Option<DateTime<Utc>>,
    pub sequence: Option<i32>,
    pub organizer: Option<Organizer>,
    pub attendees: Vec<Attendee>,
    pub reminders: Vec<Reminder>,
    pub url: Option<String>,
    pub x_properties: Vec<XProperty>,
}

impl Event {
    pub fn new(summary: impl Into<String>, start: EventTime) -> Self {
        Event {
            uid: new_uid(),
            summary: Some(summary.into()),
            description: None,
            location: None,
            start,
            end: None,
            status: None,
            transparency: None,
            recurrence: None,
            recurrence_id: None,
            last_modified: None,
            sequence: None,
            organizer: None,
            attendees: Vec::new(),
            reminders: Vec::new(),
            url: None,
            x_properties: Vec::new(),
        }
    }

    pub(crate) fn from_ics_str(contents: &str) -> Result<Self, EventError> {
        let icalendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| EventError::InvalidIcs(contents.to_string(), err))?;

        let ical_event = icalendar
            .events()
            .next()
            .ok_or_else(|| EventError::NoEventInIcs(icalendar.clone()))?;

        ical_event.try_into()
    }

    pub(crate) fn to_ics_string(&self) -> String {
        let ical_event: icalendar::Event = self.into();

        let ics = icalendar::Calendar::empty()
            .append_property(icalendar::Property::new("VERSION", ICS_VERSION))
            .append_property(icalendar::Property::new("PRODID", ICS_PRODID))
            .push(ical_event)
            .done()
            .to_string();

        self.splice_valarms_into_vevent(ics)
    }

    // icalendar library adds UID to every VALARM
    // we don't want that, so we construct them with `Reminder::ics_block` instead:
    fn splice_valarms_into_vevent(&self, ics: String) -> String {
        if self.reminders.is_empty() {
            return ics;
        }

        let valarms: String = self.reminders.iter().map(Reminder::ics_block).collect();

        ics.replacen("END:VEVENT\r\n", &format!("{valarms}END:VEVENT\r\n"), 1)
    }

    pub fn event_instance_id(&self) -> EventInstanceId {
        EventInstanceId::new(self.uid.clone(), self.recurrence_id.clone())
    }
}

// Wire format for events is ICS, not JSON. We already test the ICS round-trip
// thoroughly in `from_icalendar` / `to_icalendar`, so reusing it here means
// there's only one serialization surface to keep correct.
impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_ics_string())
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ics = String::deserialize(deserializer)?;
        Event::from_ics_str(&ics).map_err(serde::de::Error::custom)
    }
}

fn new_uid() -> EventUid {
    let uid = format!("{}@{}", uuid::Uuid::new_v4(), ICS_UID_DOMAIN);
    EventUid::from_str(uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_generates_uid_with_caldir_domain() {
        let event = Event::new(
            "Test",
            time::EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        assert!(event.uid.as_str().ends_with("@caldir"));
        let prefix = event.uid.as_str().trim_end_matches("@caldir");
        assert!(uuid::Uuid::parse_str(prefix).is_ok());
    }

    #[test]
    fn new_generates_unique_uids() {
        let start = time::EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let a = Event::new("Test", start.clone());
        let b = Event::new("Test", start);

        assert_ne!(a.uid, b.uid);
    }

    #[test]
    fn rejects_invalid_ics() {
        // Missing "END:VCALENDAR"
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT";

        let result = Event::from_ics_str(ics);
        assert!(matches!(result, Err(EventError::InvalidIcs(_, _))));
    }

    #[test]
    fn rejects_ics_without_events() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR";
        let result = Event::from_ics_str(ics);
        assert!(matches!(result, Err(EventError::NoEventInIcs(_))));
    }

    #[test]
    fn rejects_event_without_start() {
        let result = Event::try_from(&icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }

    #[test]
    fn parses_event_with_arbitrary_tzid() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART;TZID=Pacific Standard Time:20240101T120000\nSUMMARY:Test\nEND:VEVENT\nEND:VCALENDAR";

        let event = Event::from_ics_str(ics).unwrap();

        assert_eq!(
            event.start,
            EventTime::DateTimeZoned {
                datetime: chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                tzid: "Pacific Standard Time".to_string(),
            }
        );
        assert!(
            event
                .to_ics_string()
                .contains("DTSTART;TZID=Pacific Standard Time:20240101T120000")
        );
    }

    #[test]
    fn to_ics_string_sets_calendar_headers() {
        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let ics = event.to_ics_string();

        assert!(ics.contains("VERSION:2.0"));
        assert!(ics.contains("PRODID:CALDIR"));
    }

    #[test]
    fn to_ics_string_updates_dtstamp() {
        let original_ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTAMP:20200101T000000Z\r\nDTSTART:20260101\r\nSUMMARY:Test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

        let event = Event::from_ics_str(original_ics).unwrap();
        let serialized = event.to_ics_string();

        assert_ne!(dtstamp_line(original_ics), dtstamp_line(&serialized));
    }

    #[test]
    fn round_trips_advanced_event_without_data_loss() {
        let original_ics = r"BEGIN:VCALENDAR
VERSION:2.0
PRODID:CALDIR
BEGIN:VEVENT
DTSTAMP:20260502T173914Z
DESCRIPTION:https://docs.example.com/document/d/abc123def456ghijklmnopqrstu
 v/edit?usp=sharing\n
DTEND;TZID=Europe/Oslo:20260515T164500
DTSTART;TZID=Europe/Oslo:20260515T160000
LAST-MODIFIED:20260502T173914Z
LOCATION:Conference Room A
ORGANIZER:mailto:alice@example.com
RECURRENCE-ID;TZID=Europe/Oslo:20260515T160000
RRULE:FREQ=WEEKLY;BYDAY=FR
SEQUENCE:1
STATUS:CONFIRMED
SUMMARY:Friday retro
TRANSP:TRANSPARENT
UID:event-uid-123@example.com
URL:https://meet.example.com/abc-defg-hij
X-HOOLI-CONFERENCE:https://meet.example.com/abc-defg-hij
X-HOOLI-EVENT-ID:event-uid-123_20260515T140000Z
ATTENDEE;PARTSTAT=ACCEPTED:mailto:bob@example.com
ATTENDEE;PARTSTAT=DECLINED:mailto:alice@example.com
ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:carol@example.com
EXDATE;TZID=Europe/Oslo:20260522T160000
EXDATE;TZID=Europe/Oslo:20260529T160000
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:Reminder
TRIGGER;RELATED=START:-PT1H
END:VALARM
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:Reminder
TRIGGER;RELATED=START:-PT30M
END:VALARM
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let event = Event::from_ics_str(&original_ics).unwrap();
        let serialized_ics = event.to_ics_string();

        assert_eq!(strip_dtstamp(&original_ics), strip_dtstamp(&serialized_ics));
    }

    fn dtstamp_line(ics: &str) -> &str {
        ics.lines()
            .find(|line| line.starts_with("DTSTAMP:"))
            .expect("DTSTAMP line should be present")
    }

    // DTSTAMP updates every time ICS is written
    // so we need to ignore it when comparing the original and serialized ICS.
    fn strip_dtstamp(ics: &str) -> String {
        ics.lines()
            .filter(|line| !line.starts_with("DTSTAMP:"))
            .collect::<Vec<_>>()
            .join("\r\n")
            + "\r\n"
    }
}
