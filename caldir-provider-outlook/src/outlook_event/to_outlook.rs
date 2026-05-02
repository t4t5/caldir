//! Convert caldir Event to a Microsoft Graph event.

use caldir_core::event::{Event, EventTime, ParticipationStatus, Transparency};
use chrono::{Datelike, Duration, NaiveDateTime, NaiveTime, TimeZone, Utc};

use crate::graph_api::types::{
    DateTimeTimeZone, EmailAddress, GraphAttendee, GraphBody, GraphEvent, GraphLocation,
    PatternedRecurrence, RecurrencePattern, RecurrenceRange, ResponseStatus,
};

pub fn to_outlook(event: &Event) -> GraphEvent {
    let body = event.description.as_ref().map(|desc| GraphBody {
        content: desc.clone(),
        content_type: "text".to_string(),
    });

    let location = event.location.as_ref().map(|loc| GraphLocation {
        display_name: loc.clone(),
    });

    let show_as = match event.transparency {
        Transparency::Transparent => "free",
        Transparency::Opaque => "busy",
    }
    .to_string();

    let (reminder_minutes, is_reminder_on) = match event.reminders.first() {
        Some(r) => (r.minutes, true),
        None => (0, false),
    };

    let attendees = event
        .attendees
        .iter()
        .map(|a| GraphAttendee {
            email_address: EmailAddress {
                name: a.name.clone().unwrap_or_default(),
                address: a.email.clone(),
            },
            status: Some(ResponseStatus {
                response: a
                    .response_status
                    .map(participation_status_to_outlook)
                    .unwrap_or("none")
                    .to_string(),
            }),
            attendee_type: "required".to_string(),
        })
        .collect();

    let recurrence = event
        .recurrence
        .as_ref()
        .and_then(|rec| rrule_to_outlook(&rec.rrule, &event.start));

    GraphEvent {
        id: String::new(),
        i_cal_uid: String::new(),
        subject: event.summary.clone(),
        body,
        start: Some(event_time_to_graph(&event.start)),
        end: Some(event_time_to_graph(&event.end)),
        location,
        is_all_day: event.start.is_date(),
        is_cancelled: false,
        recurrence,
        attendees,
        organizer: None,
        reminder_minutes_before_start: reminder_minutes,
        is_reminder_on,
        show_as,
        last_modified_date_time: None,
        online_meeting: None,
        original_start: None,
        response_status: None,
        event_type: String::new(),
    }
}

fn event_time_to_graph(time: &EventTime) -> DateTimeTimeZone {
    match time {
        EventTime::Date(d) => DateTimeTimeZone {
            date_time: format!("{}T00:00:00.0000000", d),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeUtc(dt) => DateTimeTimeZone {
            date_time: dt.format("%Y-%m-%dT%H:%M:%S.0000000").to_string(),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeFloating(dt) => DateTimeTimeZone {
            date_time: dt.format("%Y-%m-%dT%H:%M:%S.0000000").to_string(),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeZoned { datetime, tzid } => DateTimeTimeZone {
            date_time: datetime.format("%Y-%m-%dT%H:%M:%S.0000000").to_string(),
            time_zone: iana_to_windows_timezone(tzid),
        },
    }
}

fn participation_status_to_outlook(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentativelyAccepted",
        ParticipationStatus::NeedsAction => "none",
    }
}

/// Build a `PatternedRecurrence` from an RRULE string, picking the variant
/// whose required fields match the FREQ. Each variant only carries the fields
/// its pattern type uses, so unrelated fields can't leak into the request.
fn rrule_to_outlook(rrule: &str, start: &EventTime) -> Option<PatternedRecurrence> {
    let mut freq = "";
    let mut interval = 1i32;
    let mut byday: Vec<&str> = Vec::new();
    let mut bymonthday = 0i32;
    let mut bymonth = 0i32;
    let mut until = String::new();
    let mut count = 0i32;

    for part in rrule.split(';') {
        let (key, value) = part.split_once('=')?;
        match key {
            "FREQ" => freq = value,
            "INTERVAL" => interval = value.parse().unwrap_or(1),
            "BYDAY" => byday = value.split(',').collect(),
            "BYMONTHDAY" => bymonthday = value.parse().unwrap_or(0),
            "BYMONTH" => bymonth = value.parse().unwrap_or(0),
            "UNTIL" => until = value.to_string(),
            "COUNT" => count = value.parse().unwrap_or(0),
            _ => {}
        }
    }

    let pattern = match freq {
        "DAILY" => RecurrencePattern::Daily { interval },
        "WEEKLY" => {
            let days_of_week: Vec<String> = byday
                .iter()
                .filter_map(|d| {
                    rrule_day_to_outlook(
                        d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-'),
                    )
                })
                .map(String::from)
                .collect();
            RecurrencePattern::Weekly {
                interval,
                days_of_week,
                first_day_of_week: "sunday".to_string(),
            }
        }
        "MONTHLY" if !byday.is_empty() => {
            let (index, days_of_week) = parse_relative_byday(&byday);
            RecurrencePattern::RelativeMonthly {
                interval,
                days_of_week,
                index: index.to_string(),
            }
        }
        "MONTHLY" => RecurrencePattern::AbsoluteMonthly {
            interval,
            day_of_month: if bymonthday > 0 {
                bymonthday
            } else {
                extract_day_of_month(start)
            },
        },
        "YEARLY" if !byday.is_empty() => {
            let (index, days_of_week) = parse_relative_byday(&byday);
            RecurrencePattern::RelativeYearly {
                interval,
                days_of_week,
                index: index.to_string(),
                month: if bymonth > 0 {
                    bymonth
                } else {
                    extract_month(start)
                },
            }
        }
        "YEARLY" => RecurrencePattern::AbsoluteYearly {
            interval,
            day_of_month: if bymonthday > 0 {
                bymonthday
            } else {
                extract_day_of_month(start)
            },
            month: if bymonth > 0 {
                bymonth
            } else {
                extract_month(start)
            },
        },
        _ => return None,
    };

    let start_date = match start {
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y-%m-%d").to_string(),
    };

    let range = if !until.is_empty() {
        let end_date = until_to_end_date(&until, start)?;
        RecurrenceRange::EndDate {
            start_date,
            end_date,
        }
    } else if count > 0 {
        RecurrenceRange::Numbered {
            start_date,
            number_of_occurrences: count,
        }
    } else {
        RecurrenceRange::NoEnd { start_date }
    };

    Some(PatternedRecurrence { pattern, range })
}

/// Convert an RRULE `UNTIL` value to a Graph `endDate` string ("YYYY-MM-DD").
///
/// Graph's `recurrenceRange.endDate` is day-level and inclusive, interpreted
/// in the event's start timezone. RFC 5545 `UNTIL` is an inclusive datetime,
/// often produced as `split_start - 1s` when truncating a series. Naively
/// taking the date portion of UNTIL keeps the would-be occurrence on that day
/// in the series — the bug that produced two events on the split day.
///
/// We detect that case by comparing UNTIL's local time-of-day to the start's
/// time-of-day. If UNTIL falls before the would-be occurrence on its date,
/// we step back a day so Outlook excludes that occurrence too.
fn until_to_end_date(until: &str, start: &EventTime) -> Option<String> {
    if !until.contains('T') {
        if until.len() < 8 {
            return None;
        }
        return Some(format!("{}-{}-{}", &until[..4], &until[4..6], &until[6..8]));
    }

    let local = until_local(until, start)?;
    let start_time_of_day = start_time_of_day(start);
    let occurrence_on_until_date = local.date().and_time(start_time_of_day);

    let end_date = if local < occurrence_on_until_date {
        local.date() - Duration::days(1)
    } else {
        local.date()
    };

    Some(end_date.format("%Y-%m-%d").to_string())
}

/// Interpret UNTIL in the start's timezone, returning the corresponding naive
/// local datetime. UNTIL with a `Z` suffix is UTC; without it, it's floating
/// (used as-is, since there's no timezone to convert through).
fn until_local(until: &str, start: &EventTime) -> Option<NaiveDateTime> {
    let trimmed = until.trim_end_matches('Z');
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y%m%dT%H%M%S").ok()?;

    if !until.ends_with('Z') {
        return Some(naive);
    }

    let utc = Utc.from_utc_datetime(&naive);
    let local = match start {
        EventTime::DateTimeZoned { tzid, .. } => {
            let tz: chrono_tz::Tz = tzid.parse().ok()?;
            utc.with_timezone(&tz).naive_local()
        }
        _ => utc.naive_utc(),
    };
    Some(local)
}

fn start_time_of_day(start: &EventTime) -> NaiveTime {
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    match start {
        EventTime::Date(_) => midnight,
        EventTime::DateTimeUtc(dt) => dt.time(),
        EventTime::DateTimeFloating(dt) => dt.time(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.time(),
    }
}

/// Parse BYDAY values that may have numeric prefixes (e.g., "2MO", "-1FR").
fn parse_relative_byday(byday: &[&str]) -> (&'static str, Vec<String>) {
    let mut index = "first";
    let mut days = Vec::new();

    for entry in byday {
        let entry = entry.trim();
        let (num_str, day_str) = split_byday_prefix(entry);
        if !num_str.is_empty() {
            index = number_to_outlook_index(num_str);
        }
        if let Some(day) = rrule_day_to_outlook(day_str) {
            days.push(day.to_string());
        }
    }

    (index, days)
}

fn split_byday_prefix(s: &str) -> (&str, &str) {
    let pos = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    (&s[..pos], &s[pos..])
}

fn number_to_outlook_index(n: &str) -> &'static str {
    match n {
        "1" => "first",
        "2" => "second",
        "3" => "third",
        "4" => "fourth",
        "-1" => "last",
        _ => "first",
    }
}

fn rrule_day_to_outlook(day: &str) -> Option<&'static str> {
    match day {
        "SU" => Some("sunday"),
        "MO" => Some("monday"),
        "TU" => Some("tuesday"),
        "WE" => Some("wednesday"),
        "TH" => Some("thursday"),
        "FR" => Some("friday"),
        "SA" => Some("saturday"),
        _ => None,
    }
}

fn extract_day_of_month(time: &EventTime) -> i32 {
    match time {
        EventTime::Date(d) => d.day() as i32,
        EventTime::DateTimeUtc(dt) => dt.day() as i32,
        EventTime::DateTimeFloating(dt) => dt.date().day() as i32,
        EventTime::DateTimeZoned { datetime, .. } => datetime.date().day() as i32,
    }
}

fn extract_month(time: &EventTime) -> i32 {
    match time {
        EventTime::Date(d) => d.month() as i32,
        EventTime::DateTimeUtc(dt) => dt.month() as i32,
        EventTime::DateTimeFloating(dt) => dt.date().month() as i32,
        EventTime::DateTimeZoned { datetime, .. } => datetime.date().month() as i32,
    }
}

/// Map IANA timezone names back to Windows timezone names for Graph API.
fn iana_to_windows_timezone(tz: &str) -> String {
    match tz {
        "America/New_York" => "Eastern Standard Time",
        "America/Chicago" => "Central Standard Time",
        "America/Denver" => "Mountain Standard Time",
        "America/Los_Angeles" => "Pacific Standard Time",
        "UTC" => "UTC",
        "Europe/London" => "GMT Standard Time",
        "Europe/Paris" => "Romance Standard Time",
        "Europe/Berlin" => "W. Europe Standard Time",
        "Europe/Warsaw" => "Central European Standard Time",
        "Europe/Bucharest" => "E. Europe Standard Time",
        "Europe/Helsinki" => "FLE Standard Time",
        "Europe/Athens" => "GTB Standard Time",
        "Europe/Moscow" => "Russian Standard Time",
        "Asia/Jerusalem" => "Israel Standard Time",
        "Asia/Dubai" => "Arabian Standard Time",
        "Asia/Kolkata" => "India Standard Time",
        "Asia/Shanghai" => "China Standard Time",
        "Asia/Tokyo" => "Tokyo Standard Time",
        "Asia/Seoul" => "Korea Standard Time",
        "Australia/Sydney" => "AUS Eastern Standard Time",
        "Pacific/Auckland" => "New Zealand Standard Time",
        "Pacific/Honolulu" => "Hawaiian Standard Time",
        "America/Anchorage" => "Alaskan Standard Time",
        "America/Halifax" => "Atlantic Standard Time",
        "America/Bogota" => "SA Pacific Standard Time",
        "America/Cayenne" => "SA Eastern Standard Time",
        "America/Sao_Paulo" => "E. South America Standard Time",
        "America/Buenos_Aires" => "Argentina Standard Time",
        "Asia/Bangkok" => "SE Asia Standard Time",
        "Asia/Singapore" => "Singapore Standard Time",
        "Asia/Taipei" => "Taipei Standard Time",
        "Pacific/Port_Moresby" => "West Pacific Standard Time",
        "Africa/Johannesburg" => "South Africa Standard Time",
        "Africa/Cairo" => "Egypt Standard Time",
        _ => tz, // Pass through if already Windows or unknown
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn start_on(year: i32, month: u32, day: u32) -> EventTime {
        EventTime::DateTimeUtc(
            NaiveDate::from_ymd_opt(year, month, day)
                .unwrap()
                .and_hms_opt(16, 0, 0)
                .unwrap()
                .and_utc(),
        )
    }

    // Make sure our different event times get converted properly:
    #[test]
    fn event_time_to_graph_renders_every_variant() {
        let date = EventTime::Date(NaiveDate::from_ymd_opt(2026, 5, 5).unwrap());
        let utc = EventTime::DateTimeUtc(
            NaiveDate::from_ymd_opt(2026, 5, 5)
                .unwrap()
                .and_hms_opt(11, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let floating = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 5, 5)
                .unwrap()
                .and_hms_opt(11, 0, 0)
                .unwrap(),
        );
        let zoned = EventTime::DateTimeZoned {
            datetime: NaiveDate::from_ymd_opt(2026, 5, 5)
                .unwrap()
                .and_hms_opt(11, 0, 0)
                .unwrap(),
            tzid: "Europe/London".to_string(),
        };

        assert_eq!(
            event_time_to_graph(&date).date_time,
            "2026-05-05T00:00:00.0000000"
        );
        assert_eq!(
            event_time_to_graph(&utc).date_time,
            "2026-05-05T11:00:00.0000000"
        );
        assert_eq!(
            event_time_to_graph(&floating).date_time,
            "2026-05-05T11:00:00.0000000"
        );
        let zoned_graph = event_time_to_graph(&zoned);
        assert_eq!(zoned_graph.date_time, "2026-05-05T11:00:00.0000000");
        assert_eq!(zoned_graph.time_zone, "GMT Standard Time");
    }

    #[test]
    fn daily_pattern_serializes_without_unrelated_fields() {
        // Regression: a `daily` pattern that included an empty `index` field
        // got rejected by Microsoft Graph as
        //   "Cannot parse 'null' as a value of type 'microsoft.graph.weekIndex'".
        // The tagged-enum encoding guarantees only the variant's own fields
        // appear in the JSON.
        let rec = rrule_to_outlook("FREQ=DAILY;UNTIL=20260801", &start_on(2026, 5, 1)).unwrap();
        assert!(matches!(
            rec.pattern,
            RecurrencePattern::Daily { interval: 1 }
        ));
        assert!(matches!(
            rec.range,
            RecurrenceRange::EndDate { ref start_date, ref end_date }
                if start_date == "2026-05-01" && end_date == "2026-08-01"
        ));

        let json = serde_json::to_value(&rec).unwrap();
        let pattern = json.get("pattern").unwrap().as_object().unwrap();
        assert_eq!(pattern.get("type").unwrap(), "daily");
        assert_eq!(pattern.get("interval").unwrap(), 1);
        assert!(!pattern.contains_key("index"));
        assert!(!pattern.contains_key("daysOfWeek"));
        assert!(!pattern.contains_key("dayOfMonth"));
        assert!(!pattern.contains_key("month"));
        assert!(!pattern.contains_key("firstDayOfWeek"));

        let range = json.get("range").unwrap().as_object().unwrap();
        assert!(!range.contains_key("numberOfOccurrences"));
    }

    #[test]
    fn weekly_pattern_emits_days_and_first_day_of_week() {
        let rec = rrule_to_outlook("FREQ=WEEKLY;BYDAY=MO,WE", &start_on(2026, 5, 4)).unwrap();
        match rec.pattern {
            RecurrencePattern::Weekly {
                ref days_of_week,
                ref first_day_of_week,
                ..
            } => {
                assert_eq!(days_of_week, &vec!["monday", "wednesday"]);
                assert_eq!(first_day_of_week, "sunday");
            }
            other => panic!("expected Weekly variant, got {other:?}"),
        }
    }

    #[test]
    fn relative_monthly_pattern_carries_index_and_days() {
        let rec = rrule_to_outlook("FREQ=MONTHLY;BYDAY=2MO", &start_on(2026, 5, 11)).unwrap();
        match rec.pattern {
            RecurrencePattern::RelativeMonthly {
                ref days_of_week,
                ref index,
                ..
            } => {
                assert_eq!(days_of_week, &vec!["monday"]);
                assert_eq!(index, "second");
            }
            other => panic!("expected RelativeMonthly variant, got {other:?}"),
        }
    }

    #[test]
    fn absolute_monthly_pattern_uses_day_of_month_from_start() {
        // No BYMONTHDAY in RRULE, so we extract it from the start date (15th).
        let rec = rrule_to_outlook("FREQ=MONTHLY", &start_on(2026, 5, 15)).unwrap();
        match rec.pattern {
            RecurrencePattern::AbsoluteMonthly { day_of_month, .. } => {
                assert_eq!(day_of_month, 15);
            }
            other => panic!("expected AbsoluteMonthly variant, got {other:?}"),
        }
    }

    #[test]
    fn count_range_uses_numbered_variant() {
        let rec = rrule_to_outlook("FREQ=DAILY;COUNT=10", &start_on(2026, 5, 1)).unwrap();
        assert!(matches!(
            rec.range,
            RecurrenceRange::Numbered {
                number_of_occurrences: 10,
                ..
            }
        ));
    }

    #[test]
    fn no_end_range_uses_no_end_variant() {
        let rec = rrule_to_outlook("FREQ=DAILY", &start_on(2026, 5, 1)).unwrap();
        assert!(matches!(rec.range, RecurrenceRange::NoEnd { .. }));
    }

    /// Regression: splitting a daily series at May 6 18:00 used to send
    /// `endDate = 2026-05-06`, leaving the May 6 occurrence in the truncated
    /// series — so both old and new masters showed up on May 6 in Outlook.
    /// UNTIL = `split_start - 1s` is before the would-be occurrence on its
    /// own date, so endDate should step back a day.
    #[test]
    fn until_just_before_occurrence_steps_end_date_back_one_day() {
        let rec = rrule_to_outlook(
            "FREQ=DAILY;UNTIL=20260506T175959Z",
            &EventTime::DateTimeUtc(
                NaiveDate::from_ymd_opt(2026, 5, 1)
                    .unwrap()
                    .and_hms_opt(18, 0, 0)
                    .unwrap()
                    .and_utc(),
            ),
        )
        .unwrap();
        match rec.range {
            RecurrenceRange::EndDate { end_date, .. } => assert_eq!(end_date, "2026-05-05"),
            other => panic!("expected EndDate, got {other:?}"),
        }
    }

    #[test]
    fn until_after_occurrence_keeps_end_date_same_day() {
        // UNTIL is later in the day than the start time-of-day, so the
        // occurrence on UNTIL's date is included.
        let rec = rrule_to_outlook(
            "FREQ=DAILY;UNTIL=20260506T235959Z",
            &EventTime::DateTimeUtc(
                NaiveDate::from_ymd_opt(2026, 5, 1)
                    .unwrap()
                    .and_hms_opt(8, 0, 0)
                    .unwrap()
                    .and_utc(),
            ),
        )
        .unwrap();
        match rec.range {
            RecurrenceRange::EndDate { end_date, .. } => assert_eq!(end_date, "2026-05-06"),
            other => panic!("expected EndDate, got {other:?}"),
        }
    }

    #[test]
    fn date_only_until_passes_through() {
        // Date-only UNTIL (from all-day events) is already at day-level,
        // so we just reformat without shifting.
        let rec = rrule_to_outlook(
            "FREQ=DAILY;UNTIL=20260801",
            &EventTime::Date(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
        )
        .unwrap();
        match rec.range {
            RecurrenceRange::EndDate { end_date, .. } => assert_eq!(end_date, "2026-08-01"),
            other => panic!("expected EndDate, got {other:?}"),
        }
    }

    /// Zoned start: UNTIL is in UTC but endDate must be in the start's local
    /// timezone. A NY 10:00 daily series split at May 6 → UNTIL = 13:59:59Z
    /// (= 09:59:59 NY) on May 6. The May 6 occurrence at 10:00 NY is excluded.
    #[test]
    fn until_zoned_start_compares_in_local_timezone() {
        let rec = rrule_to_outlook(
            "FREQ=DAILY;UNTIL=20260506T135959Z",
            &EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 5, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "America/New_York".to_string(),
            },
        )
        .unwrap();
        match rec.range {
            RecurrenceRange::EndDate { end_date, .. } => assert_eq!(end_date, "2026-05-05"),
            other => panic!("expected EndDate, got {other:?}"),
        }
    }
}
