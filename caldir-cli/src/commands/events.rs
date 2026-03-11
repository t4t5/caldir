use anyhow::Result;
use caldir_core::calendar::Calendar;
use chrono::{DateTime, Duration, Utc};
use owo_colors::OwoColorize;

use crate::render::{format_event_line, render_participation_status};
use crate::utils::date::format_date_only;

pub fn run(
    calendars: Vec<Calendar>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<()> {
    let start_of_today = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
        .with_timezone(&Utc);
    let from = from.unwrap_or(start_of_today);
    let to = to.unwrap_or(start_of_today + Duration::days(3));

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
        println!("{}", format_event_line(event, cal_slug, &invite_indicator));
    }

    Ok(())
}

