use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rrule::{RRuleSet, Tz as RTz};

use crate::Event;
use crate::event::{EventTime, Recurrence, RecurrenceId, Status};

/// Expand a set of events into all occurrences overlapping `[from, to)`.
pub fn expand_in_range(
    events: impl IntoIterator<Item = Event>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<Event> {
    let mut singles: Vec<Event> = Vec::new();
    let mut masters: Vec<Event> = Vec::new();
    // uid -> (recurrence_id EventTime -> override Event)
    let mut overrides: HashMap<String, HashMap<EventTime, Event>> = HashMap::new();

    for event in events {
        if event.recurrence.is_some() {
            masters.push(event);
        } else if let Some(rid) = event.recurrence_id.as_ref() {
            let uid = event.uid.as_str().to_string();
            let key = rid.as_event_time().clone();
            overrides.entry(uid).or_default().insert(key, event);
        } else {
            singles.push(event);
        }
    }

    let mut result: Vec<Event> = Vec::new();

    for event in singles {
        if event.occurs_in_range(from, to) {
            result.push(event);
        }
    }

    for master in &masters {
        let uid_overrides = overrides.remove(master.uid.as_str()).unwrap_or_default();
        result.extend(expand_master(master, from, to, &uid_overrides));
    }

    for (_uid, orphans) in overrides {
        for (_rid, event) in orphans {
            if event.occurs_in_range(from, to) {
                result.push(event);
            }
        }
    }

    result.sort_by_key(|e| e.start.to_utc());

    result
}

pub(crate) fn expand_master(
    master: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    overrides: &HashMap<EventTime, Event>,
) -> Vec<Event> {
    let Some(recurrence) = master.recurrence.as_ref() else {
        return Vec::new();
    };

    let rrule_str = build_rrule_set_string(&master.start, recurrence);
    let Ok(rrule_set) = rrule_str.parse::<RRuleSet>() else {
        return if master.occurs_in_range(from, to) {
            vec![master.clone()]
        } else {
            Vec::new()
        };
    };

    let tz: RTz = Utc.into();
    let after = from.with_timezone(&tz);
    let before = to.with_timezone(&tz);
    let dates = rrule_set.after(after).before(before).all(366).dates;

    let duration = master_duration(master);

    dates
        .into_iter()
        .filter_map(|occ| {
            let occ_time = occurrence_to_event_time(&occ, &master.start);

            if let Some(override_event) = overrides.get(&occ_time) {
                if override_event.status == Status::Cancelled {
                    return None;
                }
                return Some(override_event.clone());
            }

            Some(synthesize_instance(master, occ_time, duration))
        })
        .collect()
}

fn master_duration(master: &Event) -> Duration {
    let start = master.start.to_utc();
    let end = master.end.as_ref().map(|e| e.to_utc()).unwrap_or(start);
    end - start
}

fn synthesize_instance(master: &Event, start: EventTime, duration: Duration) -> Event {
    let end = shift_end_time(&master.start, master.end.as_ref(), &start, duration);

    Event {
        recurrence: None,
        recurrence_id: Some(RecurrenceId::from_event_time(start.clone())),
        start,
        end,
        ..master.clone()
    }
}

fn shift_end_time(
    master_start: &EventTime,
    master_end: Option<&EventTime>,
    instance_start: &EventTime,
    duration: Duration,
) -> Option<EventTime> {
    let master_end = master_end?;

    let shifted = match (master_start, master_end, instance_start) {
        (EventTime::Date(s), EventTime::Date(e), EventTime::Date(new_start)) => {
            let day_diff = (*e - *s).num_days();
            EventTime::Date(*new_start + Duration::days(day_diff))
        }
        (_, _, EventTime::DateTimeUtc(dt)) => EventTime::DateTimeUtc(*dt + duration),
        (_, _, EventTime::DateTimeFloating(dt)) => EventTime::DateTimeFloating(*dt + duration),
        (_, _, EventTime::DateTimeZoned { datetime, tzid }) => EventTime::DateTimeZoned {
            datetime: *datetime + duration,
            tzid: tzid.clone(),
        },
        // Master is timed but instance came back as a Date — preserve master_end as-is.
        (_, _, EventTime::Date(_)) => master_end.clone(),
    };

    Some(shifted)
}

fn occurrence_to_event_time(occ: &DateTime<RTz>, master_start: &EventTime) -> EventTime {
    match master_start {
        EventTime::Date(_) => EventTime::Date(occ.date_naive()),
        EventTime::DateTimeUtc(_) => EventTime::DateTimeUtc(occ.with_timezone(&Utc)),
        EventTime::DateTimeFloating(_) => EventTime::DateTimeFloating(occ.naive_local()),
        EventTime::DateTimeZoned { tzid, .. } => EventTime::DateTimeZoned {
            datetime: occ.naive_local(),
            tzid: tzid.clone(),
        },
    }
}

fn build_rrule_set_string(start: &EventTime, recurrence: &Recurrence) -> String {
    let mut lines = Vec::new();
    lines.push(format_dtstart(start));
    lines.push(format!(
        "RRULE:{}",
        normalize_until(&recurrence.rrule, start)
    ));
    for ex in &recurrence.exdates {
        lines.push(format_exdate_or_rdate("EXDATE", ex, start));
    }
    for rd in &recurrence.rdates {
        lines.push(format_exdate_or_rdate("RDATE", rd, start));
    }
    lines.join("\n")
}

fn format_dtstart(start: &EventTime) -> String {
    match start {
        EventTime::Date(d) => format!("DTSTART;VALUE=DATE:{}", d.format("%Y%m%d")),
        EventTime::DateTimeUtc(dt) => format!("DTSTART:{}", dt.format("%Y%m%dT%H%M%SZ")),
        EventTime::DateTimeFloating(dt) => format!("DTSTART:{}", dt.format("%Y%m%dT%H%M%S")),
        EventTime::DateTimeZoned { datetime, tzid } => {
            format!("DTSTART;TZID={}:{}", tzid, datetime.format("%Y%m%dT%H%M%S"))
        }
    }
}

fn format_exdate_or_rdate(name: &str, time: &EventTime, start: &EventTime) -> String {
    match (start, time) {
        (EventTime::Date(_), _) => {
            format!("{};VALUE=DATE:{}", name, time.to_utc().format("%Y%m%d"))
        }
        (_, EventTime::Date(d)) => format!("{};VALUE=DATE:{}", name, d.format("%Y%m%d")),
        (_, EventTime::DateTimeUtc(dt)) => {
            format!("{}:{}", name, dt.format("%Y%m%dT%H%M%SZ"))
        }
        (_, EventTime::DateTimeFloating(dt)) => {
            format!("{}:{}", name, dt.format("%Y%m%dT%H%M%S"))
        }
        (_, EventTime::DateTimeZoned { datetime, tzid }) => {
            format!(
                "{};TZID={}:{}",
                name,
                tzid,
                datetime.format("%Y%m%dT%H%M%S")
            )
        }
    }
}

/// The `rrule` crate requires UNTIL to share DTSTART's timezone convention:
/// UTC/zoned DTSTART → UNTIL must end with `Z`; floating/date DTSTART → no `Z`.
/// RFC 5545 actually mandates UNTIL in UTC when DTSTART is zoned, so we add a
/// `Z` for zoned starts too (the rrule crate accepts that form).
fn normalize_until(rrule: &str, start: &EventTime) -> String {
    rrule
        .split(';')
        .map(|part| {
            let Some(value) = part.strip_prefix("UNTIL=") else {
                return part.to_string();
            };

            match start {
                EventTime::DateTimeUtc(_) | EventTime::DateTimeZoned { .. } => {
                    if value.ends_with('Z') {
                        part.to_string()
                    } else if value.contains('T') {
                        format!("UNTIL={value}Z")
                    } else {
                        format!("UNTIL={value}T235959Z")
                    }
                }
                EventTime::Date(_) | EventTime::DateTimeFloating(_) => {
                    if let Some(stripped) = value.strip_suffix('Z') {
                        format!("UNTIL={stripped}")
                    } else {
                        part.to_string()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Recurrence, RecurrenceId, Status};
    use chrono::{NaiveDate, TimeZone};
    use pretty_assertions::assert_eq;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    fn timed_event(summary: &str, start: DateTime<Utc>) -> Event {
        let mut event = Event::new(summary, EventTime::DateTimeUtc(start));
        event.set_end(EventTime::DateTimeUtc(start + Duration::hours(1)));
        event
    }

    fn recurring(summary: &str, start: DateTime<Utc>, rrule: &str) -> Event {
        let mut event = timed_event(summary, start);
        event.set_recurrence(Recurrence::new(rrule));
        event
    }

    fn override_for(master: &Event, recurrence_id: EventTime) -> Event {
        let mut event = master.clone();
        event.uid = master.uid.clone();
        event.recurrence = None;
        event.recurrence_id = Some(RecurrenceId::from_event_time(recurrence_id));
        event
    }

    fn starts_at(event: &Event) -> DateTime<Utc> {
        event.start.to_utc()
    }

    #[test]
    fn returns_empty_when_no_events() {
        let from = utc(2026, 1, 1, 0, 0);
        let to = utc(2026, 2, 1, 0, 0);

        let result = expand_in_range(Vec::<Event>::new(), from, to);

        assert!(result.is_empty());
    }

    #[test]
    fn passes_through_single_event_in_range() {
        let event = timed_event("Lunch", utc(2026, 1, 15, 12, 0));
        let from = utc(2026, 1, 15, 0, 0);
        let to = utc(2026, 1, 16, 0, 0);

        let result = expand_in_range(vec![event.clone()], from, to);

        assert_eq!(result, vec![event]);
    }

    #[test]
    fn drops_single_event_outside_range() {
        let event = timed_event("Lunch", utc(2026, 2, 15, 12, 0));
        let from = utc(2026, 1, 1, 0, 0);
        let to = utc(2026, 2, 1, 0, 0);

        let result = expand_in_range(vec![event], from, to);

        assert!(result.is_empty());
    }

    #[test]
    fn expands_daily_recurrence_with_count() {
        let master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=3");
        let from = utc(2026, 1, 1, 0, 0);
        let to = utc(2026, 2, 1, 0, 0);

        let result = expand_in_range(vec![master], from, to);

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 1, 5, 9, 0),
                utc(2026, 1, 6, 9, 0),
                utc(2026, 1, 7, 9, 0),
            ]
        );
    }

    #[test]
    fn expanded_instances_carry_recurrence_id_not_rrule() {
        let master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=2");

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        assert_eq!(result.len(), 2);
        for instance in &result {
            assert!(instance.recurrence.is_none());
            assert!(instance.recurrence_id.is_some());
        }
    }

    #[test]
    fn expanded_instances_preserve_duration() {
        let mut master = Event::new("Workshop", EventTime::DateTimeUtc(utc(2026, 1, 5, 9, 0)));
        master.set_end(EventTime::DateTimeUtc(utc(2026, 1, 5, 11, 30)));
        master.set_recurrence(Recurrence::new("FREQ=WEEKLY;COUNT=2"));

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        let ends: Vec<_> = result
            .iter()
            .map(|e| e.end.as_ref().unwrap().to_utc())
            .collect();
        assert_eq!(
            ends,
            vec![utc(2026, 1, 5, 11, 30), utc(2026, 1, 12, 11, 30)]
        );
    }

    #[test]
    fn filters_recurring_instances_to_range() {
        // Daily for a year, but only 3 days fall in the requested range.
        let master = recurring("Standup", utc(2026, 1, 1, 9, 0), "FREQ=DAILY;COUNT=365");
        let from = utc(2026, 6, 1, 0, 0);
        let to = utc(2026, 6, 4, 0, 0);

        let result = expand_in_range(vec![master], from, to);

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 6, 1, 9, 0),
                utc(2026, 6, 2, 9, 0),
                utc(2026, 6, 3, 9, 0),
            ]
        );
    }

    #[test]
    fn rrule_until_bounds_expansion() {
        let mut master = recurring(
            "Daily",
            utc(2026, 1, 1, 9, 0),
            "FREQ=DAILY;UNTIL=20260103T090000Z",
        );

        // make sure normalize_until preserves a Z that's already there
        master.recurrence = Some(Recurrence::new("FREQ=DAILY;UNTIL=20260103T090000Z"));

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 1, 31, 0, 0));

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 1, 1, 9, 0),
                utc(2026, 1, 2, 9, 0),
                utc(2026, 1, 3, 9, 0),
            ]
        );
    }

    #[test]
    fn exdate_removes_specific_instance() {
        let mut master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=3");
        master.recurrence = Some(Recurrence {
            rrule: "FREQ=DAILY;COUNT=3".to_string(),
            exdates: vec![EventTime::DateTimeUtc(utc(2026, 1, 6, 9, 0))],
            rdates: vec![],
        });

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(starts, vec![utc(2026, 1, 5, 9, 0), utc(2026, 1, 7, 9, 0)]);
    }

    #[test]
    fn rdate_adds_extra_instance() {
        let mut master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=2");
        master.recurrence = Some(Recurrence {
            rrule: "FREQ=DAILY;COUNT=2".to_string(),
            exdates: vec![],
            rdates: vec![EventTime::DateTimeUtc(utc(2026, 1, 20, 9, 0))],
        });

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 1, 5, 9, 0),
                utc(2026, 1, 6, 9, 0),
                utc(2026, 1, 20, 9, 0),
            ]
        );
    }

    #[test]
    fn recurrence_id_override_replaces_generated_instance() {
        let master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=3");
        let mut override_event =
            override_for(&master, EventTime::DateTimeUtc(utc(2026, 1, 6, 9, 0)));
        override_event.summary = Some("Standup (moved)".to_string());
        override_event.start = EventTime::DateTimeUtc(utc(2026, 1, 6, 14, 0));
        override_event.end = Some(EventTime::DateTimeUtc(utc(2026, 1, 6, 15, 0)));

        let result = expand_in_range(
            vec![master, override_event],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        let summaries: Vec<_> = result
            .iter()
            .map(|e| e.summary.as_deref().unwrap())
            .collect();
        assert_eq!(summaries, vec!["Standup", "Standup (moved)", "Standup"]);
        // The overridden instance carries the moved time.
        assert_eq!(starts_at(&result[1]), utc(2026, 1, 6, 14, 0));
    }

    #[test]
    fn cancelled_override_drops_instance() {
        let master = recurring("Standup", utc(2026, 1, 5, 9, 0), "FREQ=DAILY;COUNT=3");
        let mut cancelled = override_for(&master, EventTime::DateTimeUtc(utc(2026, 1, 6, 9, 0)));
        cancelled.status = Status::Cancelled;

        let result = expand_in_range(
            vec![master, cancelled],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(starts, vec![utc(2026, 1, 5, 9, 0), utc(2026, 1, 7, 9, 0)]);
    }

    #[test]
    fn orphan_override_passes_through_if_in_range() {
        // Override with no master in the input.
        let mut event = timed_event("Adhoc instance", utc(2026, 1, 15, 10, 0));
        event.recurrence_id = Some(RecurrenceId::from_event_time(EventTime::DateTimeUtc(utc(
            2026, 1, 15, 10, 0,
        ))));

        let result = expand_in_range(
            vec![event.clone()],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        assert_eq!(result, vec![event]);
    }

    #[test]
    fn unparseable_rrule_falls_back_to_single_master_in_range() {
        let mut master = timed_event("Broken", utc(2026, 1, 15, 9, 0));
        master.set_recurrence(Recurrence::new("THIS-IS-NOT-A-VALID-RRULE"));

        let result = expand_in_range(
            vec![master.clone()],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        assert_eq!(result, vec![master]);
    }

    #[test]
    fn unparseable_rrule_returns_nothing_when_master_out_of_range() {
        let mut master = timed_event("Broken", utc(2026, 6, 15, 9, 0));
        master.set_recurrence(Recurrence::new("garbage"));

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        assert!(result.is_empty());
    }

    #[test]
    fn sorts_mixed_events_by_start_time() {
        let single_late = timed_event("Single late", utc(2026, 1, 20, 9, 0));
        let single_early = timed_event("Single early", utc(2026, 1, 2, 9, 0));
        let master = recurring("Recurring", utc(2026, 1, 10, 9, 0), "FREQ=DAILY;COUNT=2");

        let result = expand_in_range(
            vec![single_late, single_early, master],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 1, 2, 9, 0),
                utc(2026, 1, 10, 9, 0),
                utc(2026, 1, 11, 9, 0),
                utc(2026, 1, 20, 9, 0),
            ]
        );
    }

    #[test]
    fn expands_zoned_recurring_event() {
        let datetime = NaiveDate::from_ymd_opt(2026, 1, 5)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let mut master = Event::new(
            "Standup",
            EventTime::DateTimeZoned {
                datetime,
                tzid: "Europe/Stockholm".to_string(),
            },
        );
        master.set_end(EventTime::DateTimeZoned {
            datetime: datetime + Duration::minutes(30),
            tzid: "Europe/Stockholm".to_string(),
        });
        master.set_recurrence(Recurrence::new("FREQ=DAILY;COUNT=2"));

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        assert_eq!(result.len(), 2);
        for instance in &result {
            assert!(matches!(
                instance.start,
                EventTime::DateTimeZoned { ref tzid, .. } if tzid == "Europe/Stockholm"
            ));
        }
        // Stockholm is UTC+1 in January, so 09:00 local = 08:00 UTC.
        assert_eq!(starts_at(&result[0]), utc(2026, 1, 5, 8, 0));
        assert_eq!(starts_at(&result[1]), utc(2026, 1, 6, 8, 0));
    }

    #[test]
    fn all_day_recurring_event_keeps_date_variant() {
        let mut master = Event::new(
            "Holiday",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
        );
        master.set_recurrence(Recurrence::new("FREQ=WEEKLY;COUNT=2"));

        let result = expand_in_range(vec![master], utc(2026, 1, 1, 0, 0), utc(2026, 2, 1, 0, 0));

        let dates: Vec<_> = result
            .iter()
            .filter_map(|e| match e.start {
                EventTime::Date(d) => Some(d),
                _ => None,
            })
            .collect();
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 12).unwrap(),
            ]
        );
    }

    #[test]
    fn master_without_rrule_treated_as_single() {
        // Defensive: shouldn't happen in practice, but if we get a master with
        // an empty Recurrence we shouldn't crash — it just yields no instances.
        let event = timed_event("Plain", utc(2026, 1, 15, 9, 0));

        let result = expand_in_range(
            vec![event.clone()],
            utc(2026, 1, 1, 0, 0),
            utc(2026, 2, 1, 0, 0),
        );

        assert_eq!(result, vec![event]);
    }

    #[test]
    fn expands_recurring_event_parsed_from_windows_tzid_ics() {
        // rrule rejects non-IANA TZIDs, so Windows zone names in DTSTART
        // would silently collapse expansion to zero (or one) instances.
        let ics = "BEGIN:VCALENDAR\r\n\
                   VERSION:2.0\r\n\
                   BEGIN:VEVENT\r\n\
                   UID:windows-tzid-regression@caldir\r\n\
                   SUMMARY:Daily standup from Outlook\r\n\
                   DTSTART;TZID=E. South America Standard Time:20260601T090000\r\n\
                   DTEND;TZID=E. South America Standard Time:20260601T093000\r\n\
                   RRULE:FREQ=DAILY;COUNT=3\r\n\
                   END:VEVENT\r\n\
                   END:VCALENDAR\r\n";

        let master = Event::parse_single_ics(ics);

        match &master.start {
            EventTime::DateTimeZoned { tzid, .. } => assert_eq!(tzid, "America/Sao_Paulo"),
            other => panic!("expected DateTimeZoned start, got {other:?}"),
        }

        let result = expand_in_range(vec![master], utc(2026, 6, 1, 0, 0), utc(2026, 6, 5, 0, 0));

        // São Paulo is UTC-3 (no DST), so 09:00 local = 12:00 UTC.
        let starts: Vec<_> = result.iter().map(starts_at).collect();
        assert_eq!(
            starts,
            vec![
                utc(2026, 6, 1, 12, 0),
                utc(2026, 6, 2, 12, 0),
                utc(2026, 6, 3, 12, 0),
            ]
        );
    }
}
