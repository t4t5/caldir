//! Convert caldir Event to Microsoft Graph event JSON.

use caldir_core::event::{Event, EventTime, ParticipationStatus, Transparency};
use serde_json::{json, Value};

use crate::graph_types::{DateTimeTimeZone, PatternedRecurrence, RecurrencePattern, RecurrenceRange};

pub fn to_outlook(event: &Event) -> Value {
    let mut body = json!({
        "subject": event.summary,
        "start": event_time_to_graph(&event.start),
        "end": event_time_to_graph(&event.end),
        "isAllDay": event.start.is_date(),
    });

    let obj = body.as_object_mut().unwrap();

    if let Some(ref desc) = event.description {
        obj.insert(
            "body".to_string(),
            json!({ "contentType": "text", "content": desc }),
        );
    }

    if let Some(ref loc) = event.location {
        obj.insert(
            "location".to_string(),
            json!({ "displayName": loc }),
        );
    }

    // ShowAs / transparency
    let show_as = match event.transparency {
        Transparency::Transparent => "free",
        Transparency::Opaque => "busy",
    };
    obj.insert("showAs".to_string(), json!(show_as));

    // Reminders
    if let Some(reminder) = event.reminders.first() {
        obj.insert(
            "reminderMinutesBeforeStart".to_string(),
            json!(reminder.minutes),
        );
        obj.insert("isReminderOn".to_string(), json!(true));
    } else {
        obj.insert("isReminderOn".to_string(), json!(false));
    }

    // Attendees
    if !event.attendees.is_empty() {
        let attendees: Vec<Value> = event
            .attendees
            .iter()
            .map(|a| {
                json!({
                    "emailAddress": {
                        "address": a.email,
                        "name": a.name.as_deref().unwrap_or(""),
                    },
                    "type": "required",
                    "status": {
                        "response": a.response_status
                            .map(participation_status_to_outlook)
                            .unwrap_or("none"),
                    },
                })
            })
            .collect();
        obj.insert("attendees".to_string(), json!(attendees));
    }

    // Recurrence
    if let Some(ref rec) = event.recurrence
        && let Some(patterned) = rrule_to_outlook(&rec.rrule, &event.start)
    {
        obj.insert(
            "recurrence".to_string(),
            serde_json::to_value(patterned).unwrap_or(json!(null)),
        );
    }

    body
}

fn event_time_to_graph(time: &EventTime) -> DateTimeTimeZone {
    match time {
        EventTime::Date(d) => DateTimeTimeZone {
            date_time: format!("{}T00:00:00.0000000", d),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeUtc(dt) => DateTimeTimeZone {
            date_time: dt.format("%Y-%m-%dT%H:%M:%S%.7f").to_string(),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeFloating(dt) => DateTimeTimeZone {
            date_time: dt.format("%Y-%m-%dT%H:%M:%S%.7f").to_string(),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeZoned { datetime, tzid } => DateTimeTimeZone {
            date_time: datetime.format("%Y-%m-%dT%H:%M:%S%.7f").to_string(),
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

/// Convert RRULE string to Graph PatternedRecurrence.
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

    let (pattern_type, days_of_week, index, day_of_month, month) = match freq {
        "DAILY" => ("daily".to_string(), vec![], String::new(), 0, 0),
        "WEEKLY" => {
            let days: Vec<String> = byday
                .iter()
                .filter_map(|d| rrule_day_to_outlook(d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')))
                .map(String::from)
                .collect();
            ("weekly".to_string(), days, String::new(), 0, 0)
        }
        "MONTHLY" => {
            if !byday.is_empty() {
                // relativeMonthly: e.g., BYDAY=2MO
                let (index, days) = parse_relative_byday(&byday);
                ("relativeMonthly".to_string(), days, index, 0, 0)
            } else {
                let dom = if bymonthday > 0 {
                    bymonthday
                } else {
                    // Extract from start date
                    extract_day_of_month(start)
                };
                ("absoluteMonthly".to_string(), vec![], String::new(), dom, 0)
            }
        }
        "YEARLY" => {
            if !byday.is_empty() {
                let (index, days) = parse_relative_byday(&byday);
                ("relativeYearly".to_string(), days, index, 0, bymonth)
            } else {
                let dom = if bymonthday > 0 { bymonthday } else { extract_day_of_month(start) };
                let m = if bymonth > 0 { bymonth } else { extract_month(start) };
                ("absoluteYearly".to_string(), vec![], String::new(), dom, m)
            }
        }
        _ => return None,
    };

    // Range
    let (range_type, end_date, number_of_occurrences) = if !until.is_empty() {
        // Convert "20251231" or "20251231T235959Z" to "2025-12-31"
        let date_part = if until.len() >= 8 {
            format!("{}-{}-{}", &until[..4], &until[4..6], &until[6..8])
        } else {
            until.clone()
        };
        ("endDate".to_string(), date_part, 0)
    } else if count > 0 {
        ("numbered".to_string(), String::new(), count)
    } else {
        ("noEnd".to_string(), String::new(), 0)
    };

    // Start date for range
    let start_date = match start {
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y-%m-%d").to_string(),
    };

    Some(PatternedRecurrence {
        pattern: RecurrencePattern {
            pattern_type,
            interval,
            days_of_week,
            day_of_month,
            month,
            index,
            first_day_of_week: "sunday".to_string(),
        },
        range: RecurrenceRange {
            range_type,
            start_date,
            end_date,
            number_of_occurrences,
            recurrence_time_zone: String::new(),
        },
    })
}

/// Parse BYDAY values that may have numeric prefixes (e.g., "2MO", "-1FR").
fn parse_relative_byday(byday: &[&str]) -> (String, Vec<String>) {
    let mut index = "first".to_string();
    let mut days = Vec::new();

    for entry in byday {
        let entry = entry.trim();
        // Extract numeric prefix if present
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
    let pos = s
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    (&s[..pos], &s[pos..])
}

fn number_to_outlook_index(n: &str) -> String {
    match n {
        "1" => "first",
        "2" => "second",
        "3" => "third",
        "4" => "fourth",
        "-1" => "last",
        _ => "first",
    }
    .to_string()
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

use chrono::Datelike;
