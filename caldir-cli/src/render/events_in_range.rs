use anyhow::Result;
use caldir_core::{Caldir, Calendar, Event};
use chrono::{DateTime, Utc};
use owo_colors::OwoColorize;

use crate::render::event::{format_event_line, render_participation_status};
use crate::render::time::format_date_only;

pub fn render_events_in_range(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<()> {
    // (cal_slug, account_email, event)
    let mut all_events: Vec<(Option<&str>, Option<&str>, Event)> = Vec::new();

    for cal in &calendars {
        let calendar_events = cal.events_in_range(from, to)?;

        // Used to check the user's attendance status:
        let remote_email = cal.remote_email();

        for cal_event in calendar_events {
            all_events.push((cal.slug(), remote_email, cal_event.event().clone()));
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
            .filter(|email| event.is_invite_for(email))
            .and_then(|email| event.attendee_status(email))
            .map(|status| format!(" ({})", render_participation_status(status)))
            .unwrap_or_default();

        println!(
            "{}",
            format_event_line(
                event,
                cal_slug.unwrap_or("(Unknown calendar)"),
                &invite_indicator,
                caldir
            )
        );
    }

    Ok(())
}
