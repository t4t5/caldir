use caldir_core::event::EventTime;
use chrono::{DateTime, Utc};

/// Returns the start of today (midnight) in UTC.
pub fn start_of_today() -> DateTime<Utc> {
    chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
        .with_timezone(&Utc)
}

/// Format a date as a human-readable label (e.g. "Today", "Tomorrow", "Wed Feb 25")
pub fn format_date_only(time: &EventTime) -> String {
    let today = chrono::Local::now().date_naive();

    let date = match time {
        EventTime::Date(d) => *d,
        EventTime::DateTimeUtc(dt) => dt.with_timezone(&chrono::Local).date_naive(),
        EventTime::DateTimeFloating(dt) => dt.date(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.date(),
    };

    let diff = (date - today).num_days();
    match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ => date.format("%a %b %-d").to_string(),
    }
}

/// Format the time portion of an event (e.g. "  15:00" or "all-day"), right-padded to 7 chars
pub fn format_time_only(time: &EventTime) -> String {
    match time {
        EventTime::Date(_) => "all-day".to_string(),
        EventTime::DateTimeUtc(dt) => {
            format!("{:>7}", dt.with_timezone(&chrono::Local).format("%H:%M"))
        }
        EventTime::DateTimeFloating(dt) => format!("{:>7}", dt.format("%H:%M")),
        EventTime::DateTimeZoned { datetime, .. } => format!("{:>7}", datetime.format("%H:%M")),
    }
}

/// Format a compact date+time string (e.g. "Today 15:00", "Tomorrow all-day", "Wed Mar 20 15:00")
/// Used in contexts where events are not grouped by date (e.g. status/diff output).
pub fn format_datetime(time: &EventTime) -> String {
    let date_label = format_date_only(time);
    let time_label = format_time_only(time).trim_start().to_string();
    format!("{}, {}", date_label, time_label)
}
