use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use caldir_core::{Caldir, CalendarEvent, DateBounds, Event, ParticipationStatus};
use chrono::{Duration, Utc};
use owo_colors::OwoColorize;

use crate::render::event::format_event_line;
use crate::render::time::format_date_only;
use crate::utils::require_calendars;

pub fn run(caldir: &Caldir, path: Option<String>, response: Option<String>) -> Result<()> {
    require_calendars(caldir)?;

    match (path, response) {
        (Some(path), Some(response)) => run_direct(caldir, &path, &response),
        (Some(path), None) => anyhow::bail!(
            "Missing response. Usage: caldir rsvp {} accept|decline|maybe",
            path
        ),
        _ => run_interactive(caldir),
    }
}

fn run_direct(caldir: &Caldir, path_str: &str, response_str: &str) -> Result<()> {
    let path = PathBuf::from(path_str);
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }

    let cal_slug = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .context("Cannot determine calendar from path")?;

    let calendar = caldir
        .calendar(cal_slug)
        .with_context(|| format!("Failed to load calendar '{}'", cal_slug))?;

    let email = calendar
        .remote_email()
        .context("No account email configured for this calendar")?;

    let mut cal_event = CalendarEvent::load(&path).context("Failed to load event")?;
    let event = cal_event.event();

    if !event.is_invite_for(email) {
        anyhow::bail!("This event is not an invite for {}", email);
    }

    let status = parse_response(response_str)?;
    let summary = event.summary().unwrap_or("(Untitled)").to_string();
    let updated = apply_response(event, email, status)?;
    cal_event.update(updated)?;

    println!("{} {} → {}", "✓".green(), summary, status);
    println!();
    println!("{}", "Remember to run: caldir push".dimmed());

    Ok(())
}

fn run_interactive(caldir: &Caldir) -> Result<()> {
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

    // (cal_slug, email, CalendarEvent) — own the CalendarEvent so we can mutate.
    let mut invites: Vec<(String, String, CalendarEvent)> = Vec::new();

    for cal in caldir.calendars().into_iter().filter_map(Result::ok) {
        let Some(email) = cal.remote_email() else {
            continue;
        };
        let email = email.to_string();
        let cal_slug = cal.slug().unwrap_or("(Unknown calendar)").to_string();

        for ce in cal.events()? {
            let event = ce.event();
            let in_range = event.occurs_in_range(from, to);
            let is_pending = event.is_invite_for(&email)
                && event.attendee_status(&email) == Some(ParticipationStatus::NeedsAction);
            if in_range && is_pending {
                invites.push((cal_slug.clone(), email.clone(), ce));
            }
        }
    }

    invites.sort_by_key(|(_, _, ce)| ce.event().start.to_utc());

    if invites.is_empty() {
        println!("{}", "No pending invites.".dimmed());
        return Ok(());
    }

    println!(
        "Found {} pending {}.\n",
        invites.len(),
        if invites.len() == 1 {
            "invite"
        } else {
            "invites"
        }
    );

    let mut responded = 0;
    let mut current_date: Option<String> = None;

    for (cal_slug, email, mut ce) in invites {
        let event = ce.event().clone();
        let date_label = format_date_only(&event.start);
        if current_date.as_ref() != Some(&date_label) {
            if current_date.is_some() {
                println!();
            }
            println!("{}", date_label.bold());
            current_date = Some(date_label);
        }

        let organizer = event
            .organizer
            .as_ref()
            .map(|o| o.name.as_deref().unwrap_or(&o.email).to_string())
            .unwrap_or_else(|| "(unknown)".to_string());

        println!("{}", format_event_line(&event, &cal_slug, "", caldir));
        println!("       {} {}", "from:".dimmed(), organizer.dimmed());
        print!("  [a]ccept  [d]ecline  [m]aybe  [s]kip: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let status = match input {
            "a" | "accept" => Some(ParticipationStatus::Accepted),
            "d" | "decline" => Some(ParticipationStatus::Declined),
            "m" | "maybe" => Some(ParticipationStatus::Tentative),
            "s" | "skip" | "" => None,
            _ => {
                println!("  {}", "Skipping (unrecognized input)".dimmed());
                None
            }
        };

        if let Some(status) = status {
            let updated = apply_response(&event, &email, status)?;
            ce.update(updated)?;
            println!("  {} → {}", "✓".green(), status);
            responded += 1;
        } else {
            println!("  {}", "skipped".dimmed());
        }
        println!();
    }

    if responded > 0 {
        println!(
            "Updated {} {}.",
            responded,
            if responded == 1 { "invite" } else { "invites" }
        );
        println!("{}", "Remember to run: caldir push".dimmed());
    } else {
        println!("{}", "No changes made.".dimmed());
    }

    Ok(())
}

fn parse_response(input: &str) -> Result<ParticipationStatus> {
    match input.to_lowercase().as_str() {
        "a" | "accept" | "accepted" | "yes" | "y" => Ok(ParticipationStatus::Accepted),
        "d" | "decline" | "declined" | "no" | "n" => Ok(ParticipationStatus::Declined),
        "m" | "maybe" | "tentative" => Ok(ParticipationStatus::Tentative),
        other => anyhow::bail!(
            "Unknown response '{}'. Use one of: accept, decline, maybe.",
            other
        ),
    }
}

fn apply_response(event: &Event, email: &str, status: ParticipationStatus) -> Result<Event> {
    let mut updated = event.clone();

    let attendee = updated
        .attendees
        .iter_mut()
        .find(|a| a.email.eq_ignore_ascii_case(email))
        .with_context(|| format!("Not an attendee: {}", email))?;

    attendee.status = Some(status);
    updated.sequence = event.sequence + 1;
    updated.last_modified = Some(Utc::now());

    Ok(updated)
}
