use anyhow::Result as AnyhowResult;
use caldir_core::{
    Attachment, Attendee, Availability, Caldir, Calendar, Event, EventTime, Organizer,
    ParticipationStatus, Recurrence, Reminder, Status, TimeFormat, Visibility, XProperty,
};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::{Serialize, Serializer};

use crate::output::event::is_visible;
use crate::output::time::local_date;

pub struct AgendaView {
    pub entries: Vec<AgendaEntry>,
    range_start: NaiveDate,
    range_end: NaiveDate,
    time_format: TimeFormat,
}

pub struct AgendaEntry {
    pub event: Event,
    pub calendar: Option<String>,
    pub rsvp: Option<ParticipationStatus>,
}

impl AgendaView {
    pub fn collect(
        caldir: &Caldir,
        calendars: Vec<Calendar>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AnyhowResult<Self> {
        let range_start = from.with_timezone(&chrono::Local).date_naive();
        let range_end = to.with_timezone(&chrono::Local).date_naive();
        let mut entries = Vec::new();

        for calendar in calendars {
            let calendar_slug = calendar.slug().map(str::to_owned);
            let remote_email = calendar.remote_email().map(str::to_owned);

            for event in calendar.expanded_events_in_range(from, to)? {
                if !is_visible(&event) {
                    continue;
                }

                let rsvp = resolve_rsvp(&event, remote_email.as_deref());

                entries.push(AgendaEntry {
                    calendar: calendar_slug.clone(),
                    rsvp,
                    event,
                });
            }
        }

        entries.sort_by(|a, b| {
            local_date(&a.event.start)
                .cmp(&local_date(&b.event.start))
                .then_with(|| {
                    a.event
                        .start
                        .is_date()
                        .cmp(&b.event.start.is_date())
                        .reverse()
                })
                .then_with(|| a.event.start.to_utc().cmp(&b.event.start.to_utc()))
        });

        Ok(Self {
            time_format: caldir.config().time_format(),
            entries,
            range_start,
            range_end,
        })
    }

    pub(crate) fn time_format(&self) -> TimeFormat {
        self.time_format
    }

    pub(crate) fn range_start(&self) -> NaiveDate {
        self.range_start
    }

    pub(crate) fn range_end(&self) -> NaiveDate {
        self.range_end
    }
}

impl Serialize for AgendaView {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.entries.serialize(serializer)
    }
}

impl Serialize for AgendaEntry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AgendaJson::from(self).serialize(serializer)
    }
}

#[derive(Serialize)]
struct AgendaJson<'a> {
    instance_id: String,
    calendar: &'a Option<String>,
    title: &'a Option<String>,
    all_day: bool,
    tzid: Option<&'a str>,
    rsvp: Option<&'static str>,
    recurring: bool,
    #[serde(flatten)]
    event: EventJson<'a>,
}

impl<'a> From<&'a AgendaEntry> for AgendaJson<'a> {
    fn from(entry: &'a AgendaEntry) -> Self {
        Self {
            instance_id: entry.event.event_instance_id().to_string(),
            calendar: &entry.calendar,
            title: &entry.event.summary,
            all_day: entry.event.start.is_date(),
            tzid: event_tzid(&entry.event.start),
            rsvp: entry.rsvp.map(rsvp_wire_value),
            recurring: entry.event.recurrence_id.is_some(),
            event: EventJson::from(&entry.event),
        }
    }
}

#[derive(Serialize)]
struct EventJson<'a> {
    uid: &'a str,
    summary: &'a Option<String>,
    description: &'a Option<String>,
    location: &'a Option<String>,
    start: String,
    end: Option<String>,
    status: &'static str,
    availability: &'static str,
    visibility: Option<&'static str>,
    recurrence: Option<RecurrenceJson<'a>>,
    recurrence_id: Option<String>,
    organizer: Option<OrganizerJson<'a>>,
    attendees: Vec<AttendeeJson<'a>>,
    reminders: Vec<ReminderJson>,
    url: &'a Option<String>,
    attachments: Vec<AttachmentJson<'a>>,
    x_properties: Vec<XPropertyJson<'a>>,
    last_modified: Option<String>,
    sequence: i32,
}

impl<'a> From<&'a Event> for EventJson<'a> {
    fn from(event: &'a Event) -> Self {
        let Event {
            uid,
            summary,
            description,
            location,
            start,
            end,
            status,
            availability,
            visibility,
            recurrence,
            recurrence_id,
            organizer,
            attendees,
            reminders,
            url,
            attachments,
            x_properties,
            last_modified,
            sequence,
        } = event;

        Self {
            uid: uid.as_str(),
            summary,
            description,
            location,
            start: serialize_time(start),
            end: end.as_ref().map(serialize_time),
            status: status_wire_value(*status),
            availability: availability_wire_value(*availability),
            visibility: visibility.map(visibility_wire_value),
            recurrence: recurrence.as_ref().map(RecurrenceJson::from),
            recurrence_id: recurrence_id
                .as_ref()
                .map(|id| serialize_time(id.as_event_time())),
            organizer: organizer.as_ref().map(OrganizerJson::from),
            attendees: attendees.iter().map(AttendeeJson::from).collect(),
            reminders: reminders.iter().map(ReminderJson::from).collect(),
            url,
            attachments: attachments.iter().map(AttachmentJson::from).collect(),
            x_properties: x_properties.iter().map(XPropertyJson::from).collect(),
            last_modified: last_modified
                .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true)),
            sequence: *sequence,
        }
    }
}

#[derive(Serialize)]
struct RecurrenceJson<'a> {
    rrule: &'a str,
    exdates: Vec<String>,
    rdates: Vec<String>,
}

impl<'a> From<&'a Recurrence> for RecurrenceJson<'a> {
    fn from(recurrence: &'a Recurrence) -> Self {
        Self {
            rrule: &recurrence.rrule,
            exdates: recurrence.exdates.iter().map(serialize_time).collect(),
            rdates: recurrence.rdates.iter().map(serialize_time).collect(),
        }
    }
}

#[derive(Serialize)]
struct OrganizerJson<'a> {
    email: &'a str,
    name: &'a Option<String>,
}

impl<'a> From<&'a Organizer> for OrganizerJson<'a> {
    fn from(organizer: &'a Organizer) -> Self {
        Self {
            email: &organizer.email,
            name: &organizer.name,
        }
    }
}

#[derive(Serialize)]
struct AttendeeJson<'a> {
    email: &'a str,
    name: &'a Option<String>,
    status: Option<&'static str>,
}

impl<'a> From<&'a Attendee> for AttendeeJson<'a> {
    fn from(attendee: &'a Attendee) -> Self {
        Self {
            email: &attendee.email,
            name: &attendee.name,
            status: attendee.status.map(rsvp_wire_value),
        }
    }
}

#[derive(Serialize)]
struct ReminderJson {
    minutes_before_start: i64,
}

impl From<&Reminder> for ReminderJson {
    fn from(reminder: &Reminder) -> Self {
        Self {
            minutes_before_start: reminder.minutes_before_start,
        }
    }
}

#[derive(Serialize)]
struct AttachmentJson<'a> {
    uri: &'a str,
    params: Vec<PropertyParameterJson<'a>>,
}

impl<'a> From<&'a Attachment> for AttachmentJson<'a> {
    fn from(attachment: &'a Attachment) -> Self {
        Self {
            uri: &attachment.uri,
            params: property_parameters(&attachment.params),
        }
    }
}

#[derive(Serialize)]
struct XPropertyJson<'a> {
    name: &'a str,
    value: &'a str,
    params: Vec<PropertyParameterJson<'a>>,
}

impl<'a> From<&'a XProperty> for XPropertyJson<'a> {
    fn from(property: &'a XProperty) -> Self {
        Self {
            name: &property.name,
            value: &property.value,
            params: property_parameters(&property.params),
        }
    }
}

#[derive(Serialize)]
struct PropertyParameterJson<'a> {
    name: &'a str,
    value: &'a str,
}

fn property_parameters(params: &[(String, String)]) -> Vec<PropertyParameterJson<'_>> {
    params
        .iter()
        .map(|(name, value)| PropertyParameterJson { name, value })
        .collect()
}

fn resolve_rsvp(event: &Event, remote_email: Option<&str>) -> Option<ParticipationStatus> {
    remote_email
        .filter(|email| event.is_invite_for(email))
        .and_then(|email| event.attendee_status(email))
}

fn serialize_time(time: &EventTime) -> String {
    match time {
        EventTime::Date(date) => date.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(datetime) => datetime.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        EventTime::DateTimeFloating(_) => time
            .to_local_tz(&chrono::Local)
            .to_rfc3339_opts(SecondsFormat::AutoSi, false),
        EventTime::DateTimeZoned { tzid, .. } => tzid
            .parse::<chrono_tz::Tz>()
            .map(|tz| {
                time.to_local_tz(&tz)
                    .to_rfc3339_opts(SecondsFormat::AutoSi, false)
            })
            .unwrap_or_else(|_| {
                time.to_local_tz(&chrono::Local)
                    .to_rfc3339_opts(SecondsFormat::AutoSi, false)
            }),
    }
}

fn event_tzid(time: &EventTime) -> Option<&str> {
    match time {
        EventTime::DateTimeZoned { tzid, .. } => Some(tzid),
        _ => None,
    }
}

fn status_wire_value(status: Status) -> &'static str {
    match status {
        Status::Confirmed => "confirmed",
        Status::Tentative => "tentative",
        Status::Cancelled => "cancelled",
    }
}

fn availability_wire_value(availability: Availability) -> &'static str {
    match availability {
        Availability::Busy => "busy",
        Availability::Free => "free",
    }
}

fn visibility_wire_value(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Confidential => "confidential",
    }
}

fn rsvp_wire_value(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needs_action",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{Output, TextRender};
    use caldir_core::{EventUid, RecurrenceId};
    use chrono::{NaiveDateTime, TimeZone};
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn datetime(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        date(year, month, day).and_hms_opt(hour, minute, 0).unwrap()
    }

    fn entry(event: Event) -> AgendaEntry {
        AgendaEntry {
            calendar: Some("personal".to_string()),
            rsvp: None,
            event,
        }
    }

    fn view(entries: Vec<AgendaEntry>) -> AgendaView {
        AgendaView {
            time_format: TimeFormat::H24,
            entries,
            range_start: date(2026, 8, 14),
            range_end: date(2026, 8, 20),
        }
    }

    #[test]
    fn serializes_fully_populated_event() {
        let start = EventTime::DateTimeZoned {
            datetime: datetime(2026, 8, 14, 16, 0),
            tzid: "Europe/Stockholm".to_string(),
        };
        let mut event = Event::new("Friday retro", start.clone());
        event.uid = EventUid::new("retro@caldir");
        event.end = Some(EventTime::DateTimeZoned {
            datetime: datetime(2026, 8, 14, 16, 30),
            tzid: "Europe/Stockholm".to_string(),
        });
        event.location = Some("Conference room".to_string());
        event.description = Some("Weekly retrospective".to_string());
        event.status = Status::Tentative;
        event.availability = Availability::Free;
        event.visibility = Some(Visibility::Confidential);
        event.recurrence = Some(Recurrence {
            rrule: "FREQ=WEEKLY;BYDAY=FR".to_string(),
            exdates: vec![
                EventTime::Date(date(2026, 8, 21)),
                EventTime::DateTimeZoned {
                    datetime: datetime(2026, 8, 28, 16, 0),
                    tzid: "Europe/Stockholm".to_string(),
                },
            ],
            rdates: vec![EventTime::DateTimeUtc(
                Utc.with_ymd_and_hms(2026, 9, 4, 14, 0, 0).unwrap(),
            )],
        });
        event.recurrence_id = Some(RecurrenceId::from_event_time(start));
        event.organizer = Some(Organizer {
            email: "host@example.com".to_string(),
            name: Some("Host Person".to_string()),
        });
        event.attendees = vec![
            Attendee {
                email: "guest@example.com".to_string(),
                name: Some("Guest Person".to_string()),
                status: Some(ParticipationStatus::NeedsAction),
            },
            Attendee::new("observer@example.com"),
        ];
        event.reminders = vec![Reminder::from_minutes(30), Reminder::from_minutes(-5)];
        event.url = Some("https://meet.google.com/abc-defg-hij".to_string());
        event.attachments = vec![
            Attachment {
                uri: "https://example.com/agenda.html".to_string(),
                params: vec![
                    ("FMTTYPE".to_string(), "text/html".to_string()),
                    ("X-TITLE".to_string(), "Agenda".to_string()),
                ],
            },
            Attachment::new("https://example.com/notes.txt"),
        ];
        event.x_properties = vec![
            XProperty::new(
                "X-GOOGLE-CONFERENCE",
                "https://meet.google.com/abc-defg-hij",
            ),
            XProperty {
                name: "X-ALT-DESC".to_string(),
                value: "<p>Weekly retrospective</p>".to_string(),
                params: vec![
                    ("FMTTYPE".to_string(), "text/html".to_string()),
                    ("X-TITLE".to_string(), "Rich notes".to_string()),
                ],
            },
        ];
        event.last_modified = Some(Utc.with_ymd_and_hms(2026, 8, 13, 10, 11, 12).unwrap());
        event.sequence = 7;

        let mut entry = entry(event);
        entry.rsvp = Some(ParticipationStatus::Accepted);

        assert_eq!(
            serde_json::to_value(entry).unwrap(),
            json!({
                "instance_id": "retro@caldir__TZID=Europe/Stockholm:20260814T160000",
                "uid": "retro@caldir",
                "calendar": "personal",
                "title": "Friday retro",
                "summary": "Friday retro",
                "all_day": false,
                "start": "2026-08-14T16:00:00+02:00",
                "end": "2026-08-14T16:30:00+02:00",
                "tzid": "Europe/Stockholm",
                "location": "Conference room",
                "description": "Weekly retrospective",
                "status": "tentative",
                "availability": "free",
                "visibility": "confidential",
                "recurrence": {
                    "rrule": "FREQ=WEEKLY;BYDAY=FR",
                    "exdates": [
                        "2026-08-21",
                        "2026-08-28T16:00:00+02:00",
                    ],
                    "rdates": ["2026-09-04T14:00:00Z"],
                },
                "recurrence_id": "2026-08-14T16:00:00+02:00",
                "organizer": {
                    "email": "host@example.com",
                    "name": "Host Person",
                },
                "attendees": [
                    {
                        "email": "guest@example.com",
                        "name": "Guest Person",
                        "status": "needs_action",
                    },
                    {
                        "email": "observer@example.com",
                        "name": null,
                        "status": null,
                    },
                ],
                "reminders": [
                    { "minutes_before_start": 30 },
                    { "minutes_before_start": -5 },
                ],
                "url": "https://meet.google.com/abc-defg-hij",
                "attachments": [
                    {
                        "uri": "https://example.com/agenda.html",
                        "params": [
                            { "name": "FMTTYPE", "value": "text/html" },
                            { "name": "X-TITLE", "value": "Agenda" },
                        ],
                    },
                    {
                        "uri": "https://example.com/notes.txt",
                        "params": [],
                    },
                ],
                "x_properties": [
                    {
                        "name": "X-GOOGLE-CONFERENCE",
                        "value": "https://meet.google.com/abc-defg-hij",
                        "params": [],
                    },
                    {
                        "name": "X-ALT-DESC",
                        "value": "<p>Weekly retrospective</p>",
                        "params": [
                            { "name": "FMTTYPE", "value": "text/html" },
                            { "name": "X-TITLE", "value": "Rich notes" },
                        ],
                    },
                ],
                "last_modified": "2026-08-13T10:11:12Z",
                "sequence": 7,
                "rsvp": "accepted",
                "recurring": true,
            })
        );
    }

    #[test]
    fn serializes_url_and_google_conference_from_ics() {
        let ics = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:conference@caldir\r\n\
DTSTAMP:20260814T120000Z\r\n\
DTSTART:20260814T140000Z\r\n\
SUMMARY:Work meeting\r\n\
URL:https://meet.google.com/abc-defg-hij\r\n\
X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let event = Event::from_ics_str(ics)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .unwrap();

        let json = serde_json::to_value(entry(event)).unwrap();

        assert_eq!(json["url"], "https://meet.google.com/abc-defg-hij");
        assert_eq!(
            json["x_properties"],
            json!([{
                "name": "X-GOOGLE-CONFERENCE",
                "value": "https://meet.google.com/abc-defg-hij",
                "params": [],
            }])
        );
    }

    #[test]
    fn serializes_nulls_and_empty_collections() {
        let mut event = Event::new("unused", EventTime::Date(date(2026, 8, 14)));
        event.uid = EventUid::new("defaults@caldir");
        event.summary = None;

        assert_eq!(
            serde_json::to_value(entry(event)).unwrap(),
            json!({
                "instance_id": "defaults@caldir",
                "uid": "defaults@caldir",
                "calendar": "personal",
                "title": null,
                "summary": null,
                "all_day": true,
                "start": "2026-08-14",
                "end": null,
                "tzid": null,
                "location": null,
                "description": null,
                "status": "confirmed",
                "availability": "busy",
                "visibility": null,
                "recurrence": null,
                "recurrence_id": null,
                "organizer": null,
                "attendees": [],
                "reminders": [],
                "url": null,
                "attachments": [],
                "x_properties": [],
                "last_modified": null,
                "sequence": 0,
                "rsvp": null,
                "recurring": false,
            })
        );
    }

    #[test]
    fn serializes_date_only_recurrence_id() {
        let mut event = Event::new("Holiday", EventTime::Date(date(2026, 8, 14)));
        event.recurrence_id = Some(RecurrenceId::from_event_time(EventTime::Date(date(
            2026, 8, 14,
        ))));

        assert_eq!(
            serde_json::to_value(entry(event)).unwrap()["recurrence_id"],
            "2026-08-14"
        );
    }

    #[test]
    fn preserves_original_agenda_keys() {
        let mut event = Event::new("Trip", EventTime::Date(date(2026, 8, 14)));
        event.uid = EventUid::new("trip@caldir");
        event.end = Some(EventTime::Date(date(2026, 8, 17)));
        event.location = Some("Sweden".to_string());
        event.description = Some("Summer trip".to_string());
        let mut entry = entry(event);
        entry.rsvp = Some(ParticipationStatus::Tentative);

        let json = serde_json::to_value(entry).unwrap();
        let compatibility_fields = json!({
            "instance_id": "trip@caldir",
            "uid": "trip@caldir",
            "calendar": "personal",
            "title": "Trip",
            "all_day": true,
            "start": "2026-08-14",
            "end": "2026-08-17",
            "tzid": null,
            "location": "Sweden",
            "description": "Summer trip",
            "status": "confirmed",
            "rsvp": "tentative",
            "recurring": false,
        });

        for (key, value) in compatibility_fields.as_object().unwrap() {
            assert_eq!(&json[key], value, "compatibility field {key}");
        }
    }

    #[test]
    fn serializes_utc_event() {
        let start = Utc.with_ymd_and_hms(2026, 8, 14, 14, 0, 0).unwrap();
        let mut event = Event::new("UTC call", EventTime::DateTimeUtc(start));
        event.uid = EventUid::new("utc@caldir");
        event.end = Some(EventTime::DateTimeUtc(
            start + chrono::Duration::minutes(30),
        ));

        let json = serde_json::to_value(entry(event)).unwrap();

        assert_eq!(json["start"], "2026-08-14T14:00:00Z");
        assert_eq!(json["end"], "2026-08-14T14:30:00Z");
        assert_eq!(json["tzid"], serde_json::Value::Null);
        assert_eq!(json["status"], "confirmed");
        assert_eq!(json["rsvp"], serde_json::Value::Null);
        assert_eq!(json["recurring"], false);
    }

    #[test]
    fn serializes_floating_event_in_system_timezone() {
        let start = datetime(2026, 8, 14, 16, 0);
        let event = Event::new("Floating", EventTime::DateTimeFloating(start));

        let json = serde_json::to_value(entry(event)).unwrap();
        let expected = EventTime::DateTimeFloating(start)
            .to_local_tz(&chrono::Local)
            .to_rfc3339_opts(SecondsFormat::AutoSi, false);

        assert_eq!(json["start"], expected);
        assert_eq!(json["tzid"], serde_json::Value::Null);
    }

    #[test]
    fn serializes_all_day_event_with_exclusive_end() {
        let mut event = Event::new("Trip", EventTime::Date(date(2026, 8, 14)));
        event.end = Some(EventTime::Date(date(2026, 8, 17)));

        let json = serde_json::to_value(entry(event)).unwrap();

        assert_eq!(json["all_day"], true);
        assert_eq!(json["start"], "2026-08-14");
        assert_eq!(json["end"], "2026-08-17");
        assert_eq!(json["tzid"], serde_json::Value::Null);
    }

    #[test]
    fn resolves_rsvp_only_for_invites_to_calendar_account() {
        let mut event = Event::new("Invite", EventTime::Date(date(2026, 8, 14)));
        event.organizer = Some(Organizer::new("host@example.com"));
        let mut attendee = Attendee::new("me@example.com");
        attendee.status = Some(ParticipationStatus::Accepted);
        event.attendees.push(attendee);

        assert_eq!(
            resolve_rsvp(&event, Some("me@example.com")),
            Some(ParticipationStatus::Accepted)
        );
        assert_eq!(resolve_rsvp(&event, Some("other@example.com")), None);
        assert_eq!(resolve_rsvp(&event, None), None);
    }

    #[test]
    fn serializes_empty_view_as_bare_array() {
        assert_eq!(view(Vec::new()).to_json(), json!([]));
    }

    #[test]
    fn text_repeats_multi_day_event_but_json_does_not() {
        let mut event = Event::new("Trip", EventTime::Date(date(2026, 8, 14)));
        event.end = Some(EventTime::Date(date(2026, 8, 17)));
        let view = view(vec![entry(event)]);

        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.to_json().as_array().unwrap().len(), 1);
        assert_eq!(view.to_text().matches("Trip").count(), 3);
    }
}
