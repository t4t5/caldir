use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::Calendar;
use caldir_core::date_range::{parse_date_end, parse_date_start};
use chrono::{DateTime, Duration, Utc};
use owo_colors::OwoColorize;

use crate::render::{format_event_line, render_participation_status};
use crate::utils::date::{format_date_only, start_of_today};

pub fn run_week(caldir: &Caldir, calendar: Option<String>) -> Result<()> {
    require_calendars(&caldir)?;

    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;

    let today = Local::now().date_naive();

    // num_days_from_monday(): Mon=0, Tue=1, ..., Sun=6
    let days_until_sunday = (6 - today.weekday().num_days_from_monday()) % 7;

    // If today is Sunday, show through next Sunday
    let days_until_sunday = if days_until_sunday == 0 {
        7
    } else {
        days_until_sunday
    };

    let end_of_sunday = (today + chrono::Duration::days(days_until_sunday as i64))
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .with_timezone(&Utc);

    run_parsed(&caldir, calendars, from_dt, to_dt)
}

pub fn run_today(caldir: &Caldir, calendar: Option<String>) -> Result<()> {
    require_calendars(&caldir)?;

    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;

    let today = Local::now().date_naive();

    let end_of_today = today
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .with_timezone(&Utc);

    run_parsed(&caldir, calendars, from_dt, to_dt)
}

pub fn run_events(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    require_calendars(&caldir)?;

    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;

    // Only parse dates if explicitly provided; events command has its own defaults
    let from_dt = from
        .as_deref()
        .map(parse_date_start)
        .transpose()
        .map_err(|e| anyhow::anyhow!(e))?;

    let to_dt = to
        .as_deref()
        .map(parse_date_end)
        .transpose()
        .map_err(|e| anyhow::anyhow!(e))?;

    run_parsed(&caldir, calendars, from_dt, to_dt)
}

fn run_parsed(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<()> {
    let today = start_of_today();
    let from = from.unwrap_or(today);
    let to = to.unwrap_or(today + Duration::days(3));

    // (cal_slug, account_email, event)
    let mut all_events: Vec<(String, Option<String>, caldir_core::event::Event)> = Vec::new();

    for cal in &calendars {
        let email = cal.account_email().map(String::from);
        let events = cal.events_in_range(from, to)?;
        for event in events {
            all_events.push((cal.slug.clone(), email.clone(), event));
        }
    }

    // Sort by start time
    all_events.sort_by(|a, b| {
        let a_utc = a.2.start.to_utc();
        let b_utc = b.2.start.to_utc();
        a_utc.cmp(&b_utc)
    });

    if all_events.is_empty() {
        println!("{}", "No events found".dimmed());
        return Ok(());
    }

    // Group events by day and print
    let mut current_date: Option<String> = None;

    for (cal_slug, email, event) in &all_events {
        let date_label = format_date_only(&event.start);

        if current_date.as_ref() != Some(&date_label) {
            if current_date.is_some() {
                println!();
            }
            println!("{}", date_label.bold());
            current_date = Some(date_label);
        }

        let invite_indicator = email
            .as_deref()
            .filter(|e| event.is_invite_for(e))
            .and_then(|e| event.my_status(e))
            .map(|status| format!(" ({})", render_participation_status(status)))
            .unwrap_or_default();
        println!(
            "{}",
            format_event_line(event, cal_slug, &invite_indicator, caldir)
        );
    }

    Ok(())
}
