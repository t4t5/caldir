mod attendee;
mod error;
mod from_icalendar;
mod instance_id;
mod occurrences;
mod organizer;
mod recurrence;
mod reminder;
mod slugify;
mod status;
mod time;
mod to_icalendar;
mod transparency;
mod x_property;

pub use attendee::{Attendee, ParticipationStatus};
use chrono::{DateTime, Utc};
pub use error::EventError;
pub use instance_id::{EventInstanceId, EventInstanceIdError, EventUid, RecurrenceId};
pub use occurrences::expand_in_range;
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
    pub status: Status,
    pub transparency: Transparency,
    pub recurrence: Option<Recurrence>,
    pub recurrence_id: Option<RecurrenceId>,
    pub last_modified: Option<DateTime<Utc>>,
    pub sequence: i32,
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
            status: Status::default(),
            transparency: Transparency::default(),
            recurrence: None,
            recurrence_id: None,
            last_modified: None,
            sequence: 0,
            organizer: None,
            attendees: Vec::new(),
            reminders: Vec::new(),
            url: None,
            x_properties: Vec::new(),
        }
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn set_end(&mut self, end: EventTime) {
        self.end = Some(end);
    }

    pub fn set_location(&mut self, location: impl Into<String>) {
        self.location = Some(location.into());
    }

    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
    }

    pub fn set_recurrence(&mut self, recurrence: Recurrence) {
        self.recurrence = Some(recurrence);
    }

    pub fn set_reminders(&mut self, reminders: Vec<Reminder>) {
        self.reminders = reminders;
    }

    /// Parse ICS document to list of events
    pub fn from_ics_str(contents: &str) -> Result<Vec<Result<Self, EventError>>, EventError> {
        let icalendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| EventError::InvalidIcs(contents.to_string(), err))?;

        Ok(icalendar.events().map(Event::try_from).collect())
    }

    pub fn to_ics_string(&self) -> String {
        let ical_event: icalendar::Event = self.into();

        let ics = icalendar::Calendar::empty()
            .append_property(icalendar::Property::new("VERSION", ICS_VERSION))
            .append_property(icalendar::Property::new("PRODID", ICS_PRODID))
            .push(ical_event)
            .done()
            .to_string();

        self.splice_valarms_into_vevent(ics)
    }

    pub fn occurs_in_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
        let event_start = self.start.to_utc();
        let event_end = self.end.as_ref().unwrap_or(&self.start).to_utc();

        // Check if event overlaps with the range [from, to]
        event_start < to && event_end > from
    }

    /// True if email is an attendee but NOT the organizer
    pub fn is_invite_for(&self, email: &str) -> bool {
        let is_attendee = self.find_attendee(email).is_some();

        let is_organizer = self
            .organizer
            .as_ref()
            .is_some_and(|o| o.email.eq_ignore_ascii_case(email));

        is_attendee && !is_organizer
    }

    /// Get the user's participation status for this event
    pub fn attendee_status(&self, email: &str) -> Option<ParticipationStatus> {
        self.find_attendee(email)?.status
    }

    /// Find the attendee matching the given email (case-insensitive)
    pub fn find_attendee(&self, email: &str) -> Option<&Attendee> {
        self.attendees
            .iter()
            .find(|a| a.email.eq_ignore_ascii_case(email))
    }

    /// Find the first x-property value matching the given name.
    pub fn x_property(&self, name: &str) -> Option<&str> {
        self.x_properties
            .iter()
            .find(|x| x.name == name)
            .map(|x| x.value.as_str())
    }

    #[cfg(test)]
    pub(crate) fn add_x_property(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.x_properties
            .push(XProperty::new(name.into(), value.into()));

        self
    }

    /// Parse an ICS document expected to contain exactly one valid event.
    /// Panics on any deviation — for use in tests with known-good fixtures.
    #[cfg(test)]
    pub(crate) fn parse_single_ics(contents: &str) -> Event {
        let mut events = Self::from_ics_str(contents).expect("VCALENDAR should parse");
        assert_eq!(events.len(), 1, "expected exactly one event in ICS");
        events.pop().unwrap().expect("event should parse")
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

// Wire format for events is ICS, not JSON
impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_ics_string())
    }
}

// Each ICS document should have exactly one event:
impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ics = String::deserialize(deserializer)?;
        let events = Event::from_ics_str(&ics).map_err(serde::de::Error::custom)?;

        match <[Result<Event, EventError>; 1]>::try_from(events) {
            Ok([result]) => result.map_err(serde::de::Error::custom),
            Err(events) => Err(serde::de::Error::custom(format!(
                "expected exactly one event in ICS, found {}",
                events.len()
            ))),
        }
    }
}

fn new_uid() -> EventUid {
    let uid = format!("{}@{}", uuid::Uuid::new_v4(), ICS_UID_DOMAIN);
    EventUid::new(uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
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
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:test-uid@caldir
DTSTART:20240101T120000Z
SUMMARY:Test Event
END:VEVENT"
            .replace('\n', "\r\n");

        let result = Event::from_ics_str(&ics);
        assert!(matches!(result, Err(EventError::InvalidIcs(_, _))));
    }

    #[test]
    fn rejects_event_without_start() {
        let result = Event::try_from(&icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }

    #[test]
    fn parses_event_with_arbitrary_tzid() {
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:test-uid@caldir
DTSTART;TZID=Pacific Standard Time:20240101T120000
SUMMARY:Test
END:VEVENT
END:VCALENDAR"
            .replace('\n', "\r\n");

        let event = Event::parse_single_ics(&ics);

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
        let original_ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:test@caldir
DTSTAMP:20200101T000000Z
DTSTART:20260101
SUMMARY:Test
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let event = Event::parse_single_ics(&original_ics);
        let serialized = event.to_ics_string();

        assert_ne!(dtstamp_line(&original_ics), dtstamp_line(&serialized));
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

        let event = Event::parse_single_ics(&original_ics);
        let serialized_ics = event.to_ics_string();

        assert_eq!(strip_dtstamp(&original_ics), strip_dtstamp(&serialized_ics));
    }

    #[test]
    fn occurs_in_range_returns_true_for_event_inside_range() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 15, 13, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_returns_false_for_event_before_range() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 14, 12, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 14, 13, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(!event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_returns_false_for_event_after_range() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 17, 12, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 17, 13, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(!event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_returns_true_for_event_overlapping_range_start() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 14, 23, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 15, 1, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_returns_true_for_event_overlapping_range_end() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 15, 23, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 16, 1, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_uses_start_as_end_when_end_missing() {
        let event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap()),
        );

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_excludes_event_ending_exactly_at_range_start() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 14, 23, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(!event.occurs_in_range(from, to));
    }

    #[test]
    fn occurs_in_range_excludes_event_starting_exactly_at_range_end() {
        let mut event = Event::new(
            "Test",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap()),
        );
        event.set_end(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 5, 16, 1, 0, 0).unwrap(),
        ));

        let from = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 5, 16, 0, 0, 0).unwrap();

        assert!(!event.occurs_in_range(from, to));
    }

    #[test]
    fn is_invite_for_returns_true_when_attendee_and_not_organizer() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.organizer = Some(Organizer::new("alice@example.com"));
        event.attendees = vec![Attendee::new("bob@example.com")];

        assert!(event.is_invite_for("bob@example.com"));
    }

    #[test]
    fn is_invite_for_returns_false_when_email_is_organizer() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.organizer = Some(Organizer::new("alice@example.com"));
        event.attendees = vec![Attendee::new("alice@example.com")];

        assert!(!event.is_invite_for("alice@example.com"));
    }

    #[test]
    fn is_invite_for_returns_false_when_email_is_not_an_attendee() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.organizer = Some(Organizer::new("alice@example.com"));
        event.attendees = vec![Attendee::new("bob@example.com")];

        assert!(!event.is_invite_for("carol@example.com"));
    }

    #[test]
    fn is_invite_for_returns_false_when_event_has_no_attendees() {
        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        assert!(!event.is_invite_for("bob@example.com"));
    }

    #[test]
    fn is_invite_for_returns_true_when_organizer_is_missing() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.attendees = vec![Attendee::new("bob@example.com")];

        assert!(event.is_invite_for("bob@example.com"));
    }

    #[test]
    fn is_invite_for_matches_email_case_insensitively() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.organizer = Some(Organizer::new("alice@example.com"));
        event.attendees = vec![Attendee::new("Bob@Example.com")];

        assert!(event.is_invite_for("bob@example.com"));
        assert!(event.is_invite_for("BOB@EXAMPLE.COM"));
    }

    #[test]
    fn is_invite_for_matches_organizer_email_case_insensitively() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.organizer = Some(Organizer::new("Alice@Example.com"));
        event.attendees = vec![Attendee::new("alice@example.com")];

        assert!(!event.is_invite_for("ALICE@example.com"));
    }

    #[test]
    fn attendee_status_returns_status_for_matching_attendee() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.attendees = vec![Attendee {
            email: "bob@example.com".to_string(),
            name: None,
            status: Some(ParticipationStatus::Accepted),
        }];

        assert_eq!(
            event.attendee_status("bob@example.com"),
            Some(ParticipationStatus::Accepted)
        );
    }

    #[test]
    fn attendee_status_returns_none_when_email_is_not_an_attendee() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.attendees = vec![Attendee {
            email: "bob@example.com".to_string(),
            name: None,
            status: Some(ParticipationStatus::Accepted),
        }];

        assert_eq!(event.attendee_status("carol@example.com"), None);
    }

    #[test]
    fn attendee_status_returns_none_when_attendee_has_no_status() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.attendees = vec![Attendee::new("bob@example.com")];

        assert_eq!(event.attendee_status("bob@example.com"), None);
    }

    #[test]
    fn attendee_status_matches_email_case_insensitively() {
        let mut event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        event.attendees = vec![Attendee {
            email: "Bob@Example.com".to_string(),
            name: None,
            status: Some(ParticipationStatus::Tentative),
        }];

        assert_eq!(
            event.attendee_status("bob@example.com"),
            Some(ParticipationStatus::Tentative)
        );
    }

    #[test]
    fn from_ics_str_returns_empty_for_calendar_without_events() {
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
END:VCALENDAR
"
        .replace('\n', "\r\n");
        let events = Event::from_ics_str(&ics).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn from_ics_str_parses_single_event() {
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:only@caldir
DTSTART:20260301T100000Z
SUMMARY:Only event
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let events = Event::from_ics_str(&ics).unwrap();

        assert_eq!(events.len(), 1);
        let event = events.into_iter().next().unwrap().unwrap();
        assert_eq!(event.uid.as_str(), "only@caldir");
        assert_eq!(event.summary.as_deref(), Some("Only event"));
    }

    #[test]
    fn from_ics_str_returns_events_in_document_order() {
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:first@caldir
DTSTART:20260301T100000Z
SUMMARY:First
END:VEVENT
BEGIN:VEVENT
UID:second@caldir
DTSTART:20260302T100000Z
SUMMARY:Second
END:VEVENT
BEGIN:VEVENT
UID:third@caldir
DTSTART:20260303T100000Z
SUMMARY:Third
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let events: Vec<Event> = Event::from_ics_str(&ics)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect();

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].uid.as_str(), "first@caldir");
        assert_eq!(events[1].uid.as_str(), "second@caldir");
        assert_eq!(events[2].uid.as_str(), "third@caldir");
    }

    #[test]
    fn from_ics_str_handles_vtimezone_and_tzid_events() {
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTIMEZONE
TZID:Europe/Stockholm
BEGIN:STANDARD
DTSTART:19701025T030000
TZOFFSETFROM:+0200
TZOFFSETTO:+0100
RRULE:FREQ=YEARLY;BYDAY=-1SU;BYMONTH=10
TZNAME:CET
END:STANDARD
BEGIN:DAYLIGHT
DTSTART:19700329T020000
TZOFFSETFROM:+0100
TZOFFSETTO:+0200
RRULE:FREQ=YEARLY;BYDAY=-1SU;BYMONTH=3
TZNAME:CEST
END:DAYLIGHT
END:VTIMEZONE
BEGIN:VEVENT
UID:zoned@caldir
DTSTART;TZID=Europe/Stockholm:20260615T100000
SUMMARY:Zoned event
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let event = Event::parse_single_ics(&ics);

        assert!(matches!(
            event.start,
            EventTime::DateTimeZoned { ref tzid, .. } if tzid == "Europe/Stockholm"
        ));
    }

    #[test]
    fn from_ics_str_rejects_malformed_top_level() {
        // Missing END:VCALENDAR
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:x@caldir
DTSTART:20260301T100000Z
END:VEVENT
"
        .replace('\n', "\r\n");

        let result = Event::from_ics_str(&ics);

        assert!(matches!(result, Err(EventError::InvalidIcs(_, _))));
    }

    #[test]
    fn from_ics_str_surfaces_per_event_parse_errors() {
        // Second VEVENT is missing UID — surfaces as an inner Err, while the
        // first event still parses. Callers (e.g. webcal) decide whether to
        // skip or fail.
        let ics = r"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:good@caldir
DTSTART:20260301T100000Z
SUMMARY:Good
END:VEVENT
BEGIN:VEVENT
DTSTART:20260302T100000Z
SUMMARY:Missing UID
END:VEVENT
END:VCALENDAR
"
        .replace('\n', "\r\n");

        let events = Event::from_ics_str(&ics).unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].as_ref().unwrap().uid.as_str(), "good@caldir");
        assert!(matches!(events[1], Err(EventError::MissingUid)));
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
