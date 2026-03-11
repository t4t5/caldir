use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::event::EventTime;
use chrono::{Duration, Utc};
use owo_colors::OwoColorize;

use crate::render::render_participation_status;

pub fn run(calendars: Vec<Calendar>, all: bool) -> Result<()> {
    let start_of_today = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
        .with_timezone(&Utc);
    let from = start_of_today;
    let to = start_of_today + Duration::days(30);

    // (cal_slug, event, email, file_path)
    let mut invites: Vec<(String, caldir_core::event::Event, String, String)> = Vec::new();

    for cal in &calendars {
        let Some(email) = cal.account_email() else {
            continue;
        };
        let cal_events = cal.events()?;
        let events_in_range: Vec<_> = cal_events
            .into_iter()
            .filter(|ce| {
                ce.event
                    .start
                    .to_utc()
                    .is_some_and(|s| s >= from && s <= to)
            })
            .collect();

        for ce in events_in_range {
            let is_match = if all {
                ce.event.is_invite_for(email)
            } else {
                ce.event.is_pending_invite_for(email)
            };
            if is_match {
                invites.push((
                    cal.slug.clone(),
                    ce.event,
                    email.to_string(),
                    ce.path.display().to_string(),
                ));
            }
        }
    }

    // Sort by start time
    invites.sort_by(|a, b| a.1.start.to_utc().cmp(&b.1.start.to_utc()));

    if invites.is_empty() {
        println!("{}", "No pending invites.".dimmed());
        return Ok(());
    }

    // Group by day
    let mut current_date: Option<String> = None;

    for (cal_slug, event, email, path) in &invites {
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
        let status = event
            .my_status(email)
            .map(|s| format!(" ({})", render_participation_status(s)))
            .unwrap_or_default();
        let organizer = event
            .organizer
            .as_ref()
            .map(|o| {
                o.name
                    .as_deref()
                    .unwrap_or(&o.email)
                    .to_string()
            })
            .unwrap_or_default();

        println!(
            "  {} {} {}{}",
            time,
            event.summary,
            cal_tag.dimmed(),
            status
        );
        if !organizer.is_empty() {
            println!("       {} {}", "from:".dimmed(), organizer.dimmed());
        }
        println!("       {} {}", "file:".dimmed(), path.dimmed());
    }

    println!();
    println!(
        "{}",
        "Respond with: caldir rsvp <path> accept|decline|maybe".dimmed()
    );
    println!(
        "{}",
        "Or run: caldir rsvp (interactive mode)".dimmed()
    );

    Ok(())
}

/// Format a date as a human-readable label
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

/// Format the time portion of an event
fn format_time(time: &EventTime) -> String {
    match time {
        EventTime::Date(_) => "all-day".to_string(),
        EventTime::DateTimeUtc(dt) => {
            format!("{:>7}", dt.with_timezone(&chrono::Local).format("%H:%M"))
        }
        EventTime::DateTimeFloating(dt) => format!("{:>7}", dt.format("%H:%M")),
        EventTime::DateTimeZoned { datetime, .. } => format!("{:>7}", datetime.format("%H:%M")),
    }
}
