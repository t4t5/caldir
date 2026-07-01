use anyhow::Result;
use caldir_core::{Caldir, Calendar, Event, EventTime, Status, TimeFormat};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct AgendaView {
    pub days: Vec<AgendaDay>,
    #[serde(skip)]
    pub(crate) time_format: TimeFormat,
}

#[derive(Serialize)]
pub struct AgendaDay {
    pub date: NaiveDate,
    pub events: Vec<AgendaEvent>,
}

#[derive(Serialize)]
pub struct AgendaEvent {
    pub calendar_slug: Option<String>,
    pub calendar_name: Option<String>,
    pub calendar_color: Option<String>,
    pub id: String,
    pub uid: String,
    pub recurrence_id: Option<AgendaEventTime>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: AgendaEventTime,
    pub end: Option<AgendaEventTime>,
    pub status: String,
    pub invite_status: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgendaEventTime {
    Date { date: String },
    DatetimeUtc { instant: String },
    DatetimeFloating { wallclock: String },
    DatetimeZoned { wallclock: String, tzid: String },
}

struct ListedEvent<'a> {
    calendar_slug: Option<&'a str>,
    calendar_name: Option<&'a str>,
    calendar_color: Option<&'a str>,
    remote_email: Option<&'a str>,
    event: Event,
}

struct DayEvent<'a> {
    day: NaiveDate,
    listed: &'a ListedEvent<'a>,
}

impl AgendaView {
    pub fn from_range(
        caldir: &Caldir,
        calendars: Vec<Calendar>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Self> {
        let range_start = from.with_timezone(&chrono::Local).date_naive();
        let range_end = to.with_timezone(&chrono::Local).date_naive();

        let events = collect_visible_expanded_events(&calendars, from, to)?;
        let entries = group_events_by_display_day(&events, range_start, range_end);

        let mut days: Vec<AgendaDay> = Vec::new();

        for entry in entries {
            if days.last().is_none_or(|day| day.date != entry.day) {
                days.push(AgendaDay {
                    date: entry.day,
                    events: Vec::new(),
                });
            }

            days.last_mut()
                .expect("agenda day exists")
                .events
                .push(AgendaEvent::from_day_event(entry));
        }

        Ok(AgendaView {
            days,
            time_format: caldir.config().time_format(),
        })
    }
}

impl AgendaEvent {
    fn from_day_event(entry: DayEvent<'_>) -> Self {
        let listed = entry.listed;
        let event = &listed.event;
        let invite_status = listed
            .remote_email
            .filter(|email| event.is_invite_for(email))
            .and_then(|email| event.attendee_status(email));

        AgendaEvent {
            calendar_slug: listed.calendar_slug.map(str::to_string),
            calendar_name: listed.calendar_name.map(str::to_string),
            calendar_color: listed.calendar_color.map(str::to_string),
            id: event.event_instance_id().to_string(),
            uid: event.uid.as_str().to_string(),
            recurrence_id: event
                .recurrence_id
                .as_ref()
                .map(|id| AgendaEventTime::from(id.as_event_time())),
            summary: event.summary.clone(),
            description: event.description.clone(),
            location: event.location.clone(),
            start: AgendaEventTime::from(&event.start),
            end: event.end.as_ref().map(AgendaEventTime::from),
            status: event.status.to_string(),
            invite_status: invite_status.map(|status| status.to_string()),
        }
    }
}

impl AgendaEventTime {
    pub(crate) fn to_event_time(&self) -> EventTime {
        match self {
            AgendaEventTime::Date { date } => EventTime::Date(
                NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("agenda date is valid"),
            ),
            AgendaEventTime::DatetimeUtc { instant } => EventTime::DateTimeUtc(
                DateTime::parse_from_rfc3339(instant)
                    .expect("agenda instant is valid")
                    .with_timezone(&Utc),
            ),
            AgendaEventTime::DatetimeFloating { wallclock } => EventTime::DateTimeFloating(
                chrono::NaiveDateTime::parse_from_str(wallclock, "%Y-%m-%dT%H:%M:%S")
                    .expect("agenda floating datetime is valid"),
            ),
            AgendaEventTime::DatetimeZoned { wallclock, tzid } => EventTime::DateTimeZoned {
                datetime: chrono::NaiveDateTime::parse_from_str(wallclock, "%Y-%m-%dT%H:%M:%S")
                    .expect("agenda zoned datetime is valid"),
                tzid: tzid.clone(),
            },
        }
    }
}

impl From<&EventTime> for AgendaEventTime {
    fn from(value: &EventTime) -> Self {
        match value {
            EventTime::Date(date) => AgendaEventTime::Date {
                date: date.format("%Y-%m-%d").to_string(),
            },
            EventTime::DateTimeUtc(datetime) => AgendaEventTime::DatetimeUtc {
                instant: datetime.to_rfc3339(),
            },
            EventTime::DateTimeFloating(datetime) => AgendaEventTime::DatetimeFloating {
                wallclock: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
            },
            EventTime::DateTimeZoned { datetime, tzid } => AgendaEventTime::DatetimeZoned {
                wallclock: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                tzid: tzid.clone(),
            },
        }
    }
}

fn collect_visible_expanded_events<'a>(
    calendars: &'a [Calendar],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<ListedEvent<'a>>> {
    let mut events = Vec::new();

    for cal in calendars {
        let calendar_events = cal.expanded_events_in_range(from, to)?;
        let remote_email = cal.remote_email();

        for event in calendar_events {
            if event.status != Status::Cancelled {
                events.push(ListedEvent {
                    calendar_slug: cal.slug(),
                    calendar_name: cal.name(),
                    calendar_color: cal.color(),
                    remote_email,
                    event,
                });
            }
        }
    }

    events.sort_by(|a, b| {
        a.event
            .start
            .is_date()
            .cmp(&b.event.start.is_date())
            .reverse()
            .then_with(|| a.event.start.to_utc().cmp(&b.event.start.to_utc()))
    });

    Ok(events)
}

fn group_events_by_display_day<'a>(
    events: &'a [ListedEvent<'a>],
    range_start: NaiveDate,
    range_end: NaiveDate,
) -> Vec<DayEvent<'a>> {
    let mut entries = Vec::new();

    for event in events {
        for day in display_days(&event.event, range_start, range_end) {
            entries.push(DayEvent { day, listed: event });
        }
    }

    entries.sort_by(|a, b| {
        a.day
            .cmp(&b.day)
            .then_with(|| {
                a.listed
                    .event
                    .start
                    .is_date()
                    .cmp(&b.listed.event.start.is_date())
                    .reverse()
            })
            .then_with(|| {
                a.listed
                    .event
                    .start
                    .to_utc()
                    .cmp(&b.listed.event.start.to_utc())
            })
    });

    entries
}

/// The day(s) an event should be listed under, clamped to `[range_start, range_end]`.
/// Most events render once, on their start day.
/// A multi-day all-day event renders under every day it covers
fn display_days(event: &Event, range_start: NaiveDate, range_end: NaiveDate) -> Vec<NaiveDate> {
    if let (EventTime::Date(start), Some(EventTime::Date(end))) = (&event.start, &event.end) {
        // All-day DTEND is exclusive, so the last day covered is `end - 1`.
        let last_day = *end - Duration::days(1);
        if last_day > *start {
            let first = (*start).max(range_start);
            let last = last_day.min(range_end);
            let mut days = Vec::new();
            let mut day = first;
            while day <= last {
                days.push(day);
                day += Duration::days(1);
            }
            return days;
        }
    }

    vec![local_date(&event.start)]
}

fn local_date(time: &EventTime) -> NaiveDate {
    match time {
        EventTime::Date(d) => *d,
        EventTime::DateTimeUtc(dt) => dt.with_timezone(&chrono::Local).date_naive(),
        EventTime::DateTimeFloating(dt) => dt.date(),
        EventTime::DateTimeZoned { datetime, tzid } => {
            if let Ok(tz) = tzid.parse::<chrono_tz::Tz>()
                && let Some(zoned) = datetime.and_local_timezone(tz).single()
            {
                return zoned.with_timezone(&chrono::Local).date_naive();
            }
            datetime.date()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn all_day(start: NaiveDate, end_exclusive: NaiveDate) -> Event {
        let mut event = Event::new("Trip", EventTime::Date(start));
        event.end = Some(EventTime::Date(end_exclusive));
        event
    }

    fn agenda_event(summary: &str) -> AgendaEvent {
        AgendaEvent {
            calendar_slug: Some("work".to_string()),
            calendar_name: Some("Work".to_string()),
            calendar_color: Some("#ff0000".to_string()),
            id: "event-1".to_string(),
            uid: "uid-1".to_string(),
            recurrence_id: None,
            summary: Some(summary.to_string()),
            description: None,
            location: None,
            start: AgendaEventTime::Date {
                date: "2027-05-27".to_string(),
            },
            end: None,
            status: "confirmed".to_string(),
            invite_status: None,
        }
    }

    #[test]
    fn agenda_view_serializes_grouped_json() {
        let day = date(2027, 5, 27);
        let view = AgendaView {
            days: vec![AgendaDay {
                date: day,
                events: vec![agenda_event("Trip")],
            }],
            time_format: TimeFormat::H24,
        };

        let json = serde_json::to_value(view).unwrap();

        assert_eq!(json["days"][0]["date"], "2027-05-27");
        assert_eq!(json["days"][0]["events"][0]["summary"], "Trip");
        assert_eq!(json["days"][0]["events"][0]["calendar_slug"], "work");
        assert!(json["days"][0]["events"][0].get("text_line").is_none());
        assert!(json["time_format"].is_null());
    }

    #[test]
    fn single_day_all_day_event_shows_on_its_start_day() {
        // Spans one day (DTEND is exclusive): May 27 only.
        let event = all_day(date(2026, 5, 27), date(2026, 5, 28));

        let days = display_days(&event, date(2026, 5, 25), date(2026, 6, 1));

        assert_eq!(days, vec![date(2026, 5, 27)]);
    }

    #[test]
    fn multi_day_all_day_event_shows_on_every_spanned_day() {
        // May 27 through May 29 inclusive (DTEND May 30 exclusive).
        let event = all_day(date(2026, 5, 27), date(2026, 5, 30));

        let days = display_days(&event, date(2026, 5, 25), date(2026, 6, 1));

        assert_eq!(
            days,
            vec![date(2026, 5, 27), date(2026, 5, 28), date(2026, 5, 29)]
        );
    }

    #[test]
    fn multi_day_event_starting_before_window_is_clamped_to_window_start() {
        // The reported bug: trip began May 27 but today is June 2. It should
        // appear from the window start onward, not under the past start day.
        let event = all_day(date(2026, 5, 27), date(2026, 6, 5));

        let days = display_days(&event, date(2026, 6, 2), date(2026, 6, 7));

        assert_eq!(
            days,
            vec![date(2026, 6, 2), date(2026, 6, 3), date(2026, 6, 4)]
        );
    }

    #[test]
    fn multi_day_event_extending_past_window_is_clamped_to_window_end() {
        let event = all_day(date(2026, 6, 1), date(2026, 6, 20));

        let days = display_days(&event, date(2026, 6, 1), date(2026, 6, 3));

        assert_eq!(
            days,
            vec![date(2026, 6, 1), date(2026, 6, 2), date(2026, 6, 3)]
        );
    }

    #[test]
    fn timed_event_shows_only_on_its_start_day() {
        let mut event = Event::new(
            "Meeting",
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 6, 2, 14, 0, 0).unwrap()),
        );
        event.end = Some(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 6, 2, 15, 0, 0).unwrap(),
        ));

        let days = display_days(&event, date(2026, 6, 1), date(2026, 6, 7));

        assert_eq!(days, vec![local_date(&event.start)]);
    }
}
