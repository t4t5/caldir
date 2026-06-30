use caldir_core::{EventTime, ParticipationStatus};
use serde::Serialize;

use crate::render::events_in_range::DayEvent;

#[derive(Serialize)]
pub struct JsonEvent<'a> {
    pub display_day: String,
    pub calendar_slug: Option<&'a str>,
    pub calendar_name: Option<&'a str>,
    pub calendar_color: Option<&'a str>,
    pub id: String,
    pub uid: &'a str,
    pub recurrence_id: Option<JsonEventTime>,
    pub summary: Option<&'a str>,
    pub description: Option<&'a str>,
    pub location: Option<&'a str>,
    pub start: JsonEventTime,
    pub end: Option<JsonEventTime>,
    pub status: String,
    pub invite_status: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JsonEventTime {
    Date { date: String },
    DatetimeUtc { instant: String },
    DatetimeFloating { wallclock: String },
    DatetimeZoned { wallclock: String, tzid: String },
}

impl From<&EventTime> for JsonEventTime {
    fn from(value: &EventTime) -> Self {
        match value {
            EventTime::Date(date) => JsonEventTime::Date {
                date: date.format("%Y-%m-%d").to_string(),
            },
            EventTime::DateTimeUtc(datetime) => JsonEventTime::DatetimeUtc {
                instant: datetime.to_rfc3339(),
            },
            EventTime::DateTimeFloating(datetime) => JsonEventTime::DatetimeFloating {
                wallclock: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
            },
            EventTime::DateTimeZoned { datetime, tzid } => JsonEventTime::DatetimeZoned {
                wallclock: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                tzid: tzid.clone(),
            },
        }
    }
}

impl<'a> From<&'a DayEvent<'a>> for JsonEvent<'a> {
    fn from(value: &'a DayEvent<'a>) -> Self {
        let listed = value.listed;
        let event = &listed.event;
        let invite_status = listed
            .remote_email
            .filter(|email| event.is_invite_for(email))
            .and_then(|email| event.attendee_status(email))
            .map(participation_status_label);

        JsonEvent {
            display_day: value.day.format("%Y-%m-%d").to_string(),
            calendar_slug: listed.calendar_slug,
            calendar_name: listed.calendar_name,
            calendar_color: listed.calendar_color,
            id: event.event_instance_id().to_string(),
            uid: event.uid.as_str(),
            recurrence_id: event
                .recurrence_id
                .as_ref()
                .map(|id| JsonEventTime::from(id.as_event_time())),
            summary: event.summary.as_deref(),
            description: event.description.as_deref(),
            location: event.location.as_deref(),
            start: JsonEventTime::from(&event.start),
            end: event.end.as_ref().map(JsonEventTime::from),
            status: event.status.to_string(),
            invite_status,
        }
    }
}

fn participation_status_label(status: ParticipationStatus) -> String {
    status.to_string()
}
