use caldir_core::{EventTime, TimeFormat};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

/// The local calendar date an event time falls on.
pub fn local_date(time: &EventTime) -> NaiveDate {
    match time {
        EventTime::Date(d) => *d,
        EventTime::DateTimeUtc(dt) => dt.with_timezone(&chrono::Local).date_naive(),
        EventTime::DateTimeFloating(dt) => dt.date(),
        EventTime::DateTimeZoned { datetime, tzid } => zoned_to_local(datetime, tzid).date(),
    }
}

/// Format a date as a human-readable label (e.g. "Today", "Tomorrow", "Wed Feb 25").
/// The year is appended when the date is not in the current year (e.g. "Wed Feb 25 2023").
pub fn format_date_label(date: NaiveDate) -> String {
    let today = chrono::Local::now().date_naive();

    let diff = (date - today).num_days();
    match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ if date.year() == today.year() => date.format("%a %b %-d").to_string(),
        _ => date.format("%a %b %-d %Y").to_string(),
    }
}

/// Format an event time as a human-readable date label.
pub fn format_date_only(time: &EventTime) -> String {
    format_date_label(local_date(time))
}

/// Format the time portion of an event (e.g. "  15:00" or " 3:00pm" or "all-day"), right-padded to 7 chars
pub fn format_time_only(time: &EventTime, time_format: TimeFormat) -> String {
    match time {
        EventTime::Date(_) => "all-day".to_string(),
        EventTime::DateTimeUtc(dt) => {
            let local = dt.with_timezone(&chrono::Local).naive_local();
            format_naive_time(&local, time_format)
        }
        EventTime::DateTimeFloating(dt) => format_naive_time(dt, time_format),
        EventTime::DateTimeZoned { datetime, tzid } => {
            format_naive_time(&zoned_to_local(datetime, tzid), time_format)
        }
    }
}

// Format a compact date+time string (e.g. "Today 15:00", "Tomorrow all-day", "Wed Mar 20 15:00")
// Used in contexts where events are not grouped by date (e.g. status/diff output).
pub fn format_datetime(time: &EventTime, time_format: TimeFormat) -> String {
    let date_label = format_date_only(time);
    let time_label = format_time_only(time, time_format).trim_start().to_string();
    format!("{}, {}", date_label, time_label)
}

/// Convert a zoned datetime to the system's local NaiveDateTime.
/// Falls back to the original datetime if the timezone can't be parsed.
fn zoned_to_local(datetime: &NaiveDateTime, tzid: &str) -> NaiveDateTime {
    if let Ok(tz) = tzid.parse::<chrono_tz::Tz>()
        && let Some(zoned) = datetime.and_local_timezone(tz).single()
    {
        return zoned.with_timezone(&chrono::Local).naive_local();
    }
    *datetime
}

/// Format a NaiveDateTime's time portion according to the given format.
fn format_naive_time(dt: &NaiveDateTime, time_format: TimeFormat) -> String {
    match time_format {
        TimeFormat::H24 => format!("{:>7}", dt.format("%H:%M")),
        TimeFormat::H12 => {
            let hour = dt.hour();
            let minute = dt.minute();
            let (h12, ampm) = if hour == 0 {
                (12, "am")
            } else if hour < 12 {
                (hour, "am")
            } else if hour == 12 {
                (12, "pm")
            } else {
                (hour - 12, "pm")
            };
            format!("{:>7}", format!("{}:{:02}{}", h12, minute, ampm))
        }
    }
}
