use anyhow::Result as AnyhowResult;
use caldir_core::{Caldir, Calendar, Event, EventTime, ParticipationStatus, Status, TimeFormat};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::ser::{Serialize, SerializeSeq, SerializeStruct, Serializer};

use crate::render::event::is_visible;
use crate::render::time::local_date;

pub struct AgendaView {
    time_format: TimeFormat,
    pub entries: Vec<AgendaEntry>,
    range_start: NaiveDate,
    range_end: NaiveDate,
}

pub struct AgendaEntry {
    pub calendar: Option<String>,
    pub rsvp: Option<ParticipationStatus>,
    pub event: Event,
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
        let mut seq = serializer.serialize_seq(Some(self.entries.len()))?;
        for entry in &self.entries {
            seq.serialize_element(entry)?;
        }
        seq.end()
    }
}

impl Serialize for AgendaEntry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AgendaEntry", 13)?;
        state.serialize_field("instance_id", &self.event.event_instance_id().to_string())?;
        state.serialize_field("uid", self.event.uid.as_str())?;
        state.serialize_field("calendar", &self.calendar)?;
        state.serialize_field("title", &self.event.summary)?;
        state.serialize_field("all_day", &self.event.start.is_date())?;
        state.serialize_field("start", &serialize_time(&self.event.start))?;
        state.serialize_field("end", &self.event.end.as_ref().map(serialize_time))?;
        state.serialize_field("tzid", &event_tzid(&self.event.start))?;
        state.serialize_field("location", &self.event.location)?;
        state.serialize_field("description", &self.event.description)?;
        state.serialize_field("status", status_wire_value(self.event.status))?;
        state.serialize_field("rsvp", &self.rsvp.map(rsvp_wire_value))?;
        state.serialize_field("recurring", &self.event.recurrence_id.is_some())?;
        state.end()
    }
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
    use caldir_core::{Attendee, EventUid, Organizer, RecurrenceId};
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
    fn serializes_zoned_recurring_invite() {
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
        event.recurrence_id = Some(RecurrenceId::from_event_time(start));

        let mut entry = entry(event);
        entry.rsvp = Some(ParticipationStatus::Accepted);

        assert_eq!(
            serde_json::to_value(entry).unwrap(),
            json!({
                "instance_id": "retro@caldir__TZID=Europe/Stockholm:20260814T160000",
                "uid": "retro@caldir",
                "calendar": "personal",
                "title": "Friday retro",
                "all_day": false,
                "start": "2026-08-14T16:00:00+02:00",
                "end": "2026-08-14T16:30:00+02:00",
                "tzid": "Europe/Stockholm",
                "location": "Conference room",
                "description": "Weekly retrospective",
                "status": "tentative",
                "rsvp": "accepted",
                "recurring": true,
            })
        );
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
