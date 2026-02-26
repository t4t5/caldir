//! RRULE expansion for recurring events.
//!
//! Expands a master recurring event into individual instances within a date range,
//! respecting EXDATEs and instance overrides from disk.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rrule::RRuleSet;

use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventTime, Recurrence};

/// Ensure the UNTIL parameter in an RRULE string matches the DTSTART timezone convention.
///
/// The rrule crate validates that DTSTART and UNTIL use the same timezone form:
/// - UTC DTSTART → UNTIL must end with Z
/// - Floating DTSTART → UNTIL must not have Z
/// - Zoned DTSTART (TZID) → UNTIL must be expressed in the same named timezone (no Z)
///
/// RFC 5545 specifies UNTIL in UTC when DTSTART is zoned, but the rrule crate
/// requires them to match. For zoned events, we convert the UTC UNTIL to the
/// DTSTART's timezone.
fn normalize_rrule_until(rrule: &str, start: &EventTime) -> String {
    rrule
        .split(';')
        .map(|part| {
            if !part.starts_with("UNTIL=") {
                return part.to_string();
            }
            let value = &part[6..];

            match start {
                EventTime::DateTimeUtc(_) => {
                    // DTSTART is UTC — UNTIL must also end with Z
                    if !value.ends_with('Z') {
                        if value.contains('T') {
                            format!("UNTIL={}Z", value)
                        } else {
                            format!("UNTIL={}T235959Z", value)
                        }
                    } else {
                        part.to_string()
                    }
                }
                EventTime::DateTimeZoned { .. } => {
                    // DTSTART has a named timezone — rrule crate expects UNTIL in UTC
                    if !value.ends_with('Z') {
                        if value.contains('T') {
                            format!("UNTIL={}Z", value)
                        } else {
                            format!("UNTIL={}T235959Z", value)
                        }
                    } else {
                        part.to_string()
                    }
                }
                _ => {
                    // Date / Floating — UNTIL must not have Z
                    if value.ends_with('Z') {
                        format!("UNTIL={}", value.trim_end_matches('Z'))
                    } else {
                        part.to_string()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Build an iCalendar-format RRULE string for the rrule crate parser.
fn build_rrule_string(start: &EventTime, recurrence: &Recurrence) -> String {
    let mut lines = Vec::new();

    // DTSTART — Date and Floating stay floating (no Z) so they match
    // their RRULE's UNTIL format. UTC keeps Z. Zoned keeps TZID.
    let dtstart = match start {
        EventTime::Date(d) => {
            format!("DTSTART:{}T000000", d.format("%Y%m%d"))
        }
        EventTime::DateTimeUtc(dt) => {
            format!("DTSTART:{}", dt.format("%Y%m%dT%H%M%SZ"))
        }
        EventTime::DateTimeFloating(dt) => {
            format!("DTSTART:{}", dt.format("%Y%m%dT%H%M%S"))
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

    // RRULE — normalize UNTIL to match DTSTART's timezone convention
    let rrule = normalize_rrule_until(&recurrence.rrule, start);
    lines.push(format!("RRULE:{}", rrule));

    // EXDATE lines — must also match DTSTART's timezone convention
    for exdate in &recurrence.exdates {
        let exdate_str = match exdate {
            EventTime::Date(d) => format!("EXDATE:{}T000000", d.format("%Y%m%d")),
            EventTime::DateTimeUtc(dt) => {
                format!("EXDATE:{}", dt.format("%Y%m%dT%H%M%SZ"))
            }
            EventTime::DateTimeFloating(dt) => {
                format!("EXDATE:{}", dt.format("%Y%m%dT%H%M%S"))
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
    // The rrule crate's `all()` uses inclusive boundaries on both ends.
    let tz: rrule::Tz = Utc.into();
    let after = range_start.with_timezone(&tz);
    let before = range_end.with_timezone(&tz);

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    /// Reproduce the exact failing event from disk:
    /// DTSTART;TZID=Europe/Stockholm:19680508T000000
    /// RRULE:FREQ=YEARLY;UNTIL=20080507T120000Z
    #[test]
    fn test_zoned_dtstart_with_utc_until() {
        let start = EventTime::DateTimeZoned {
            datetime: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1968, 5, 8).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
            tzid: "Europe/Stockholm".to_string(),
        };
        let recurrence = Recurrence {
            rrule: "FREQ=YEARLY;UNTIL=20080507T120000Z".to_string(),
            exdates: vec![],
        };

        let rrule_str = build_rrule_string(&start, &recurrence);
        eprintln!("Generated rrule string:\n{}", rrule_str);

        // This is the line that was failing:
        let result: Result<RRuleSet, _> = rrule_str.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    /// Reproduce the all-day event case:
    /// DTSTART;VALUE=DATE:20080508
    /// RRULE:FREQ=YEARLY;UNTIL=20220507
    #[test]
    fn test_allday_dtstart_with_date_until() {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2008, 5, 8).unwrap());
        let recurrence = Recurrence {
            rrule: "FREQ=YEARLY;UNTIL=20220507".to_string(),
            exdates: vec![],
        };

        let rrule_str = build_rrule_string(&start, &recurrence);
        eprintln!("Generated rrule string:\n{}", rrule_str);

        let result: Result<RRuleSet, _> = rrule_str.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }
}
