use std::io::{self, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::event::ParticipationStatus;

use crate::render::format_event_line;
use crate::utils::date::format_date_label;
use caldir_core::ics::parse_event;
use chrono::{Duration, Utc};
use owo_colors::OwoColorize;

pub fn run(path: Option<String>, response: Option<String>) -> Result<()> {
    match (path, response) {
        (Some(path), Some(response)) => run_direct(&path, &response),
        (Some(path), None) => {
            anyhow::bail!(
                "Missing response. Usage: caldir rsvp {} accept|decline|maybe",
                path
            );
        }
        _ => run_interactive(),
    }
}

fn run_direct(path_str: &str, response_str: &str) -> Result<()> {
    let path = PathBuf::from(path_str);
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }

    let content = std::fs::read_to_string(&path).context("Failed to read ICS file")?;
    let event = parse_event(&content).context("Failed to parse ICS file")?;

    // Determine calendar slug from parent directory
    let cal_dir = path
        .parent()
        .context("Cannot determine calendar directory")?;
    let cal_slug = cal_dir
        .file_name()
        .and_then(|n| n.to_str())
        .context("Cannot determine calendar slug")?;

    let calendar = Calendar::load(cal_slug)
        .context(format!("Failed to load calendar '{}'", cal_slug))?;

    let email = calendar
        .account_email()
        .context("No account email configured for this calendar")?;

    if !event.is_invite_for(email) {
        anyhow::bail!("This event is not an invite for {}", email);
    }

    let status = ParticipationStatus::from_str(response_str)
        .map_err(|e| anyhow::anyhow!(e))?;

    let updated_event = event
        .with_response(email, status)
        .context("Failed to update event response")?;

    calendar.update_event(&updated_event.uid, &updated_event)?;

    println!(
        "{} {} → {}",
        "✓".green(),
        event.summary,
        status
    );
    println!();
    println!(
        "{}",
        "Remember to run: caldir push".dimmed()
    );

    Ok(())
}

fn run_interactive() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    let start_of_today = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
        .with_timezone(&Utc);
    let from = start_of_today;
    let to = start_of_today + Duration::days(30);

    // Collect pending invites: (calendar, event, email, path)
    let mut invites: Vec<(Calendar, caldir_core::event::Event, String, PathBuf)> = Vec::new();

    for cal in &calendars {
        let Some(email) = cal.account_email() else {
            continue;
        };
        let cal_events = cal.events()?;
        for ce in cal_events {
            let in_range = ce
                .event
                .start
                .to_utc()
                .is_some_and(|s| s >= from && s <= to);
            if in_range && ce.event.is_pending_invite_for(email) {
                invites.push((cal.clone(), ce.event, email.to_string(), ce.path));
            }
        }
    }

    invites.sort_by(|a, b| a.1.start.to_utc().cmp(&b.1.start.to_utc()));

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

    for (calendar, event, email, _path) in &invites {
        let date_label = format_date_label(&event.start);
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

        println!("{}", format_event_line(event, &calendar.slug, ""));
        println!(
            "       {} {}",
            "from:".dimmed(),
            organizer.dimmed()
        );
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
            let updated = event
                .with_response(email, status)
                .context("Failed to update response")?;
            calendar.update_event(&updated.uid, &updated)?;
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
        println!(
            "{}",
            "Remember to run: caldir push".dimmed()
        );
    } else {
        println!("{}", "No changes made.".dimmed());
    }

    Ok(())
}

