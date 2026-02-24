use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::event::EventTime;
use chrono::{DateTime, Duration, Utc};
use owo_colors::OwoColorize;


pub fn run(calendars: Vec<Calendar>, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Result<()> {
    let now = Utc::now();
    let from = from.unwrap_or(now);
    let to = to.unwrap_or(now + Duration::days(3));

    let mut all_events: Vec<(String, caldir_core::event::Event)> = Vec::new();

    for cal in &calendars {
        let events = cal.events_in_range(from, to)?;
        for event in events {
            all_events.push((cal.slug.clone(), event));
        }
    }

    // Sort by start time
    all_events.sort_by(|a, b| {
        let a_utc = a.1.start.to_utc();
        let b_utc = b.1.start.to_utc();
        a_utc.cmp(&b_utc)
    });

    if all_events.is_empty() {
        println!("{}", "No events found".dimmed());
        return Ok(());
    }

    // Group events by day and print
    let mut current_date: Option<String> = None;

    for (cal_slug, event) in &all_events {
        let date_label = format_date_label(&event.start);

        if current_date.as_ref() != Some(&date_label) {
            if current_date.is_some() {
                println!();
            }
            println!("{}", date_label.bold());
            current_date = Some(date_label);
        }

        let time = format_time(&event.start);
        let cal_tag = format!("[{}]", cal_slug);
        println!("  {} {} {}", time, event.summary, cal_tag.dimmed());
    }

    Ok(())
}

/// Format a date as a human-readable label (e.g. "Today", "Tomorrow", "Wed Feb 25")
fn format_date_label(time: &EventTime) -> String {
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

/// Format the time portion of an event (e.g. "15:00" or "all-day")
fn format_time(time: &EventTime) -> String {
    match time {
        EventTime::Date(_) => "all-day".to_string(),
        EventTime::DateTimeUtc(dt) => format!("{:>7}", dt.with_timezone(&chrono::Local).format("%H:%M")),
        EventTime::DateTimeFloating(dt) => format!("{:>7}", dt.format("%H:%M")),
        EventTime::DateTimeZoned { datetime, .. } => format!("{:>7}", datetime.format("%H:%M")),
    }
}
