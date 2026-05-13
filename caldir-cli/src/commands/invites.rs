use anyhow::Result;
use caldir_core::{Caldir, DateBounds, ParticipationStatus};
use chrono::{Duration, Utc};
use owo_colors::OwoColorize;

use crate::render::event::{format_event_line, render_participation_status};
use crate::render::time::format_date_only;
use crate::utils::{require_calendars, resolve_calendars};

pub fn run(caldir: &Caldir, calendar: Option<String>, all: bool) -> Result<()> {
    require_calendars(caldir)?;
    let calendars = resolve_calendars(caldir, calendar.as_deref())?;

    let tz: chrono_tz::Tz = iana_time_zone::get_timezone()?.parse()?;
    let today = Utc::now().with_timezone(&tz).date_naive();

    let from = today
        .start_of_date()
        .and_local_timezone(tz)
        .earliest()
        .unwrap()
        .with_timezone(&Utc);

    let to = (today + Duration::days(30))
        .end_of_date()
        .and_local_timezone(tz)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    let mut invites: Vec<(String, caldir_core::Event, String)> = Vec::new();

    for cal in &calendars {
        let Some(email) = cal.remote_email() else {
            continue;
        };

        let events = cal.expanded_events_in_range(from, to)?;
        let cal_slug = cal.slug().unwrap_or("(Unknown calendar)").to_string();

        for event in events {
            let is_invite = event.is_invite_for(email);
            let matches = if all {
                is_invite
            } else {
                is_invite && event.attendee_status(email) == Some(ParticipationStatus::NeedsAction)
            };
            if matches {
                invites.push((cal_slug.clone(), event, email.to_string()));
            }
        }
    }

    invites.sort_by_key(|(_, event, _)| event.start.to_utc());

    if invites.is_empty() {
        println!("{}", "No pending invites.".dimmed());
        return Ok(());
    }

    let mut current_date: Option<String> = None;

    for (cal_slug, event, email) in &invites {
        let date_label = format_date_only(&event.start);
        if current_date.as_ref() != Some(&date_label) {
            if current_date.is_some() {
                println!();
            }
            println!("{}", date_label.bold());
            current_date = Some(date_label);
        }

        let status_suffix = event
            .attendee_status(email)
            .map(|s| format!(" ({})", render_participation_status(s)))
            .unwrap_or_default();
        println!(
            "{}",
            format_event_line(event, cal_slug, &status_suffix, caldir)
        );

        if let Some(organizer) = event.organizer.as_ref().filter(|o| !o.email.is_empty()) {
            println!("       {} {}", "from:".dimmed(), organizer.email.dimmed());
        }
    }

    println!();
    println!("Respond with: caldir rsvp");

    Ok(())
}
