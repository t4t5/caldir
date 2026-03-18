use std::sync::LazyLock;

use caldir_core::caldir::Caldir;
use caldir_core::caldir_config::TimeFormat;
use caldir_core::event::EventTime;
use chrono::{DateTime, NaiveDateTime, Timelike, Utc};

/// Lazily loaded time format from global config. Defaults to 24h if config can't be read.
static TIME_FORMAT: LazyLock<TimeFormat> = LazyLock::new(|| {
    Caldir::load()
        .map(|c| c.config().time_format)
        .unwrap_or_default()
});

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
        EventTime::DateTimeZoned { datetime, tzid } => zoned_to_local(datetime, tzid).date(),
    };

    let diff = (date - today).num_days();
    match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ => date.format("%a %b %-d").to_string(),
    }
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

/// Format the time portion of an event (e.g. "  15:00" or " 3:00pm" or "all-day"), right-padded to 7 chars
pub fn format_time_only(time: &EventTime) -> String {
    match time {
        EventTime::Date(_) => "all-day".to_string(),
        EventTime::DateTimeUtc(dt) => {
            let local = dt.with_timezone(&chrono::Local).naive_local();
            format_naive_time(&local, *TIME_FORMAT)
        }
        EventTime::DateTimeFloating(dt) => format_naive_time(dt, *TIME_FORMAT),
        EventTime::DateTimeZoned { datetime, tzid } => {
            format_naive_time(&zoned_to_local(datetime, tzid), *TIME_FORMAT)
        }
    }
}

/// Format a compact date+time string (e.g. "Today 15:00", "Tomorrow all-day", "Wed Mar 20 15:00")
/// Used in contexts where events are not grouped by date (e.g. status/diff output).
pub fn format_datetime(time: &EventTime) -> String {
    let date_label = format_date_only(time);
    let time_label = format_time_only(time).trim_start().to_string();
    format!("{}, {}", date_label, time_label)
}
