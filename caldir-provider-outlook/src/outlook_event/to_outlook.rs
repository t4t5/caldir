//! Convert caldir Event to Microsoft Graph event JSON.

use caldir_core::event::{Event, EventTime, ParticipationStatus, Transparency};
use serde_json::{Value, json};

use crate::graph_types::DateTimeTimeZone;

pub fn to_outlook(event: &Event) -> Value {
    let mut body = json!({
        "subject": event.summary,
        "start": event_time_to_graph(&event.start),
        "end": event_time_to_graph(&event.end),
        "isAllDay": event.start.is_date(),
    });

    // Safety: json!({...}) always returns Value::Object
    let Some(obj) = body.as_object_mut() else {
        return body;
    };

    if let Some(ref desc) = event.description {
        obj.insert(
            "body".to_string(),
            json!({ "contentType": "text", "content": desc }),
        );
    }

    if let Some(ref loc) = event.location {
        obj.insert("location".to_string(), json!({ "displayName": loc }));
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
        && let Some(recurrence) = rrule_to_outlook(&rec.rrule, &event.start)
    {
        obj.insert("recurrence".to_string(), recurrence);
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
            date_time: dt.format("%Y-%m-%dT%H:%M:%S.0000000").to_string(),
            time_zone: "UTC".to_string(),
        },
        EventTime::DateTimeFloating(dt) => DateTimeTimeZone {
            date_time: dt.format("%Y-%m-%dT%H:%M:%S.0000000").to_string(),
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

/// Convert an RRULE string to Microsoft Graph's `recurrence` JSON shape.
///
/// Graph has six distinct pattern types (`daily`, `weekly`,
/// `absoluteMonthly`, `relativeMonthly`, `absoluteYearly`, `relativeYearly`)
/// with disjoint required fields — sending fields that don't apply (e.g.
/// `index` on a daily pattern) gets rejected as an invalid `weekIndex`. We
/// build the JSON directly so each call only emits the fields its pattern
/// type uses.
fn rrule_to_outlook(rrule: &str, start: &EventTime) -> Option<Value> {
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
        "DAILY" => json!({
            "type": "daily",
            "interval": interval,
        }),
        "WEEKLY" => {
            let days: Vec<&str> = byday
                .iter()
                .filter_map(|d| {
                    rrule_day_to_outlook(
                        d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-'),
                    )
                })
                .collect();
            json!({
                "type": "weekly",
                "interval": interval,
                "daysOfWeek": days,
                "firstDayOfWeek": "sunday",
            })
        }
        "MONTHLY" if !byday.is_empty() => {
            let (index, days) = parse_relative_byday(&byday);
            json!({
                "type": "relativeMonthly",
                "interval": interval,
                "daysOfWeek": days,
                "index": index,
            })
        }
        "MONTHLY" => {
            let day_of_month = if bymonthday > 0 {
                bymonthday
            } else {
                extract_day_of_month(start)
            };
            json!({
                "type": "absoluteMonthly",
                "interval": interval,
                "dayOfMonth": day_of_month,
            })
        }
        "YEARLY" if !byday.is_empty() => {
            let (index, days) = parse_relative_byday(&byday);
            let month = if bymonth > 0 {
                bymonth
            } else {
                extract_month(start)
            };
            json!({
                "type": "relativeYearly",
                "interval": interval,
                "daysOfWeek": days,
                "index": index,
                "month": month,
            })
        }
        "YEARLY" => {
            let day_of_month = if bymonthday > 0 {
                bymonthday
            } else {
                extract_day_of_month(start)
            };
            let month = if bymonth > 0 {
                bymonth
            } else {
                extract_month(start)
            };
            json!({
                "type": "absoluteYearly",
                "interval": interval,
                "dayOfMonth": day_of_month,
                "month": month,
            })
        }
        _ => return None,
    };

    let start_date = match start {
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y-%m-%d").to_string(),
    };

    let range = if !until.is_empty() {
        // Convert "20251231" or "20251231T235959Z" to "2025-12-31"
        let end_date = if until.len() >= 8 {
            format!("{}-{}-{}", &until[..4], &until[4..6], &until[6..8])
        } else {
            until.clone()
        };
        json!({
            "type": "endDate",
            "startDate": start_date,
            "endDate": end_date,
        })
    } else if count > 0 {
        json!({
            "type": "numbered",
            "startDate": start_date,
            "numberOfOccurrences": count,
        })
    } else {
        json!({
            "type": "noEnd",
            "startDate": start_date,
        })
    };

    Some(json!({ "pattern": pattern, "range": range }))
}

/// Parse BYDAY values that may have numeric prefixes (e.g., "2MO", "-1FR").
fn parse_relative_byday(byday: &[&str]) -> (&'static str, Vec<&'static str>) {
    let mut index = "first";
    let mut days = Vec::new();

    for entry in byday {
        let entry = entry.trim();
        let (num_str, day_str) = split_byday_prefix(entry);
        if !num_str.is_empty() {
            index = number_to_outlook_index(num_str);
        }
        if let Some(day) = rrule_day_to_outlook(day_str) {
            days.push(day);
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

use chrono::Datelike;

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

    #[test]
    fn daily_pattern_omits_index_and_other_unrelated_fields() {
        // Regression: a `daily` pattern that included an empty `index` field
        // got rejected by Microsoft Graph as
        //   "Cannot parse 'null' as a value of type 'microsoft.graph.weekIndex'".
        // Each pattern type must only emit fields that apply to it.
        let json = rrule_to_outlook("FREQ=DAILY;UNTIL=20260801", &start_on(2026, 5, 1)).unwrap();
        let pattern = json.get("pattern").unwrap().as_object().unwrap();
        assert_eq!(pattern.get("type").unwrap(), "daily");
        assert_eq!(pattern.get("interval").unwrap(), 1);
        assert!(!pattern.contains_key("index"));
        assert!(!pattern.contains_key("daysOfWeek"));
        assert!(!pattern.contains_key("dayOfMonth"));
        assert!(!pattern.contains_key("month"));
        assert!(!pattern.contains_key("firstDayOfWeek"));

        let range = json.get("range").unwrap().as_object().unwrap();
        assert_eq!(range.get("type").unwrap(), "endDate");
        assert_eq!(range.get("startDate").unwrap(), "2026-05-01");
        assert_eq!(range.get("endDate").unwrap(), "2026-08-01");
        assert!(!range.contains_key("numberOfOccurrences"));
        assert!(!range.contains_key("recurrenceTimeZone"));
    }

    #[test]
    fn weekly_pattern_emits_days_and_first_day_of_week() {
        let json = rrule_to_outlook("FREQ=WEEKLY;BYDAY=MO,WE", &start_on(2026, 5, 4)).unwrap();
        let pattern = json.get("pattern").unwrap().as_object().unwrap();
        assert_eq!(pattern.get("type").unwrap(), "weekly");
        assert_eq!(
            pattern.get("daysOfWeek").unwrap(),
            &json!(["monday", "wednesday"])
        );
        assert_eq!(pattern.get("firstDayOfWeek").unwrap(), "sunday");
        assert!(!pattern.contains_key("index"));
        assert!(!pattern.contains_key("dayOfMonth"));
    }

    #[test]
    fn relative_monthly_pattern_emits_index_and_days() {
        let json = rrule_to_outlook("FREQ=MONTHLY;BYDAY=2MO", &start_on(2026, 5, 11)).unwrap();
        let pattern = json.get("pattern").unwrap().as_object().unwrap();
        assert_eq!(pattern.get("type").unwrap(), "relativeMonthly");
        assert_eq!(pattern.get("index").unwrap(), "second");
        assert_eq!(pattern.get("daysOfWeek").unwrap(), &json!(["monday"]));
        assert!(!pattern.contains_key("dayOfMonth"));
        assert!(!pattern.contains_key("firstDayOfWeek"));
    }

    #[test]
    fn absolute_monthly_pattern_uses_day_of_month_from_start() {
        // No BYMONTHDAY in RRULE, so we extract it from the start date (15th).
        let json = rrule_to_outlook("FREQ=MONTHLY", &start_on(2026, 5, 15)).unwrap();
        let pattern = json.get("pattern").unwrap().as_object().unwrap();
        assert_eq!(pattern.get("type").unwrap(), "absoluteMonthly");
        assert_eq!(pattern.get("dayOfMonth").unwrap(), 15);
        assert!(!pattern.contains_key("daysOfWeek"));
        assert!(!pattern.contains_key("index"));
    }

    #[test]
    fn count_range_uses_numbered_type() {
        let json = rrule_to_outlook("FREQ=DAILY;COUNT=10", &start_on(2026, 5, 1)).unwrap();
        let range = json.get("range").unwrap().as_object().unwrap();
        assert_eq!(range.get("type").unwrap(), "numbered");
        assert_eq!(range.get("numberOfOccurrences").unwrap(), 10);
        assert!(!range.contains_key("endDate"));
    }

    #[test]
    fn no_end_range_omits_count_and_end_date() {
        let json = rrule_to_outlook("FREQ=DAILY", &start_on(2026, 5, 1)).unwrap();
        let range = json.get("range").unwrap().as_object().unwrap();
        assert_eq!(range.get("type").unwrap(), "noEnd");
        assert!(!range.contains_key("endDate"));
        assert!(!range.contains_key("numberOfOccurrences"));
    }
}
