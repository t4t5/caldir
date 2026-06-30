use anyhow::Result;
use caldir_core::{Caldir, Calendar, Event, EventTime};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use owo_colors::OwoColorize;

use crate::render::event::{format_event_line, is_visible, render_participation_status};
use crate::render::time::{format_date_label, local_date};

struct ListedEvent<'a> {
    calendar_slug: Option<&'a str>,
    remote_email: Option<&'a str>,
    event: Event,
}

struct DayEvent<'a> {
    day: NaiveDate,
    listed: &'a ListedEvent<'a>,
}

pub fn render_text_events_in_range(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<()> {
    let range_start = from.with_timezone(&chrono::Local).date_naive();
    let range_end = to.with_timezone(&chrono::Local).date_naive();

    let events = collect_visible_expanded_events(&calendars, from, to)?;
    let entries = group_events_by_display_day(&events, range_start, range_end);

    if entries.is_empty() {
        println!("{}", "No events found".dimmed());
        return Ok(());
    }

    // Group events by day and print
    let mut current_date: Option<NaiveDate> = None;

    for entry in &entries {
        let day = entry.day;
        let listed = entry.listed;
        let event = &listed.event;

        if current_date != Some(day) {
            if current_date.is_some() {
                println!();
            }
            println!("{}", format_date_label(day).bold());
            current_date = Some(day);
        }

        let invite_indicator = listed
            .remote_email
            .filter(|email| event.is_invite_for(email))
            .and_then(|email| event.attendee_status(email))
            .map(|status| format!(" ({})", render_participation_status(status)))
            .unwrap_or_default();

        println!(
            "{}",
            format_event_line(
                event,
                listed.calendar_slug.unwrap_or("(Unknown calendar)"),
                &invite_indicator,
                caldir
            )
        );
    }

    Ok(())
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
            if is_visible(&event) {
                events.push(ListedEvent {
                    calendar_slug: cal.slug(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn all_day(start: NaiveDate, end_exclusive: NaiveDate) -> Event {
        let mut event = Event::new("Trip", EventTime::Date(start));
        event.end = Some(EventTime::Date(end_exclusive));
        event
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
