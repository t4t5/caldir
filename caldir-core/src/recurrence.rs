//! RRULE expansion for recurring events.
//!
//! Expands a master recurring event into individual instances within a date range,
//! respecting EXDATEs and instance overrides from disk.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rrule::RRuleSet;

use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventTime, Recurrence};

/// Build an iCalendar-format RRULE string for the rrule crate parser.
fn build_rrule_string(start: &EventTime, recurrence: &Recurrence) -> String {
    let mut lines = Vec::new();

    // DTSTART — the rrule crate needs a datetime, so all-day dates become midnight UTC
    let dtstart = match start {
        EventTime::Date(d) => {
            format!("DTSTART:{}T000000Z", d.format("%Y%m%d"))
        }
        EventTime::DateTimeUtc(dt) => {
            format!("DTSTART:{}", dt.format("%Y%m%dT%H%M%SZ"))
        }
        EventTime::DateTimeFloating(dt) => {
            format!("DTSTART:{}Z", dt.format("%Y%m%dT%H%M%S"))
        }
        EventTime::DateTimeZoned { datetime, tzid } => {
            format!(
                "DTSTART;TZID={}:{}",
                tzid,
                datetime.format("%Y%m%dT%H%M%S")
            )
        }
    };
    lines.push(dtstart);

    // RRULE
    lines.push(format!("RRULE:{}", recurrence.rrule));

    // EXDATE lines
    for exdate in &recurrence.exdates {
        let exdate_str = match exdate {
            EventTime::Date(d) => format!("EXDATE:{}T000000Z", d.format("%Y%m%d")),
            EventTime::DateTimeUtc(dt) => {
                format!("EXDATE:{}", dt.format("%Y%m%dT%H%M%SZ"))
            }
            EventTime::DateTimeFloating(dt) => {
                format!("EXDATE:{}Z", dt.format("%Y%m%dT%H%M%S"))
            }
            EventTime::DateTimeZoned { datetime, tzid } => {
                format!(
                    "EXDATE;TZID={}:{}",
                    tzid,
                    datetime.format("%Y%m%dT%H%M%S")
                )
            }
        };
        lines.push(exdate_str);
    }

    lines.join("\n")
}

/// Convert an rrule occurrence datetime back to an EventTime matching the master's variant.
fn occurrence_to_event_time(dt: &DateTime<rrule::Tz>, master_start: &EventTime) -> EventTime {
    match master_start {
        EventTime::Date(_) => EventTime::Date(dt.date_naive()),
        EventTime::DateTimeUtc(_) => EventTime::DateTimeUtc(dt.with_timezone(&Utc)),
        EventTime::DateTimeFloating(_) => EventTime::DateTimeFloating(dt.naive_utc()),
        EventTime::DateTimeZoned { tzid, .. } => EventTime::DateTimeZoned {
            datetime: dt.naive_local(),
            tzid: tzid.clone(),
        },
    }
}

/// Expand a recurring master event into individual instances within [range_start, range_end].
///
/// - `overrides` maps recurrence-id ICS strings to override Events (instance exceptions from disk).
///   If an override exists for a given occurrence, it replaces the generated instance.
/// - The master event itself is NOT included; only expanded instances with `recurrence_id` set.
pub fn expand_recurring_event(
    master: &Event,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    overrides: &HashMap<String, Event>,
) -> CalDirResult<Vec<Event>> {
    let recurrence = match &master.recurrence {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let rrule_str = build_rrule_string(&master.start, recurrence);

    let rrule_set: RRuleSet = rrule_str.parse().map_err(|e| {
        CalDirError::IcsParse(format!(
            "Failed to parse RRULE for event '{}': {}",
            master.uid, e
        ))
    })?;

    // Convert range boundaries to rrule's Tz type.
    // Subtract/add 1 second to make the range inclusive (after/before are exclusive).
    let tz: rrule::Tz = Utc.into();
    let after = (range_start - Duration::seconds(1)).with_timezone(&tz);
    let before = (range_end + Duration::seconds(1)).with_timezone(&tz);

    let result = rrule_set.after(after).before(before).all(365);

    // Calculate master event duration
    let duration = match (master.start.to_utc(), master.end.to_utc()) {
        (Some(s), Some(e)) => e - s,
        _ => Duration::zero(),
    };

    let mut events = Vec::new();

    for occ_dt in &result.dates {
        let occ_event_time = occurrence_to_event_time(occ_dt, &master.start);
        let ics_key = occ_event_time.to_ics_string();

        if let Some(override_event) = overrides.get(&ics_key) {
            events.push(override_event.clone());
        } else {
            // Build instance end time preserving the master's EventTime variant
            let instance_end = match (&master.start, &master.end) {
                (EventTime::Date(d_start), EventTime::Date(d_end)) => {
                    let day_diff = (*d_end - *d_start).num_days();
                    EventTime::Date(occ_dt.date_naive() + Duration::days(day_diff))
                }
                (EventTime::DateTimeUtc(_), _) => {
                    EventTime::DateTimeUtc(occ_dt.with_timezone(&Utc) + duration)
                }
                (EventTime::DateTimeFloating(_), _) => {
                    EventTime::DateTimeFloating(occ_dt.naive_utc() + duration)
                }
                (EventTime::DateTimeZoned { tzid, .. }, _) => EventTime::DateTimeZoned {
                    datetime: occ_dt.naive_local() + duration,
                    tzid: tzid.clone(),
                },
                // Fallback: use UTC
                _ => EventTime::DateTimeUtc(occ_dt.with_timezone(&Utc) + duration),
            };

            events.push(Event {
                uid: master.uid.clone(),
                summary: master.summary.clone(),
                description: master.description.clone(),
                location: master.location.clone(),
                start: occ_event_time.clone(),
                end: instance_end,
                status: master.status.clone(),
                recurrence: None,
                recurrence_id: Some(occ_event_time),
                reminders: master.reminders.clone(),
                transparency: master.transparency.clone(),
                organizer: master.organizer.clone(),
                attendees: master.attendees.clone(),
                conference_url: master.conference_url.clone(),
                updated: master.updated,
                sequence: master.sequence,
                custom_properties: master.custom_properties.clone(),
            });
        }
    }

    Ok(events)
}
