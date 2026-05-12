use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::Calendar;
use caldir_core::DateRange;
use caldir_core::{BatchDiff, CalendarDiff};
use owo_colors::OwoColorize;

use crate::commands::guards::allow_mass_delete;
use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    require_calendars(&caldir)?;
    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;
    let range =
        DateRange::from_args(from.as_deref(), to.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    run_parsed(&caldir, calendars, range, verbose, force).await
}

async fn run_parsed(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    range: DateRange,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        if cal.remote().is_none() {
            println!("{}", cal.render(caldir));
            println!("   {}", "(local only)".dimmed());
        } else {
            let spinner = tui::create_spinner(cal.render(caldir));
            let result = CalendarDiff::from_calendar(caldir, cal, &range).await;
            spinner.finish_and_clear();

            println!("{}", cal.render(caldir));

            match result {
                Ok(diff) => {
                    println!("{}", diff.render_sync(verbose, caldir));
                    diff.apply_pull()?;
                    if !allow_mass_delete(&diff, force) {
                        continue;
                    }
                    diff.apply_push().await?;
                    diffs.push(diff);
                }
                Err(e) => println!("   {}", e.to_string().red()),
            }
        }

        if i < calendars.len() - 1 {
            println!();
        }
    }

    let batch = BatchDiff(diffs);
    let (pull_created, pull_updated, pull_deleted) = batch.pull_counts();
    let (push_created, push_updated, push_deleted) = batch.push_counts();

    if pull_created > 0 || pull_updated > 0 || pull_deleted > 0 {
        println!(
            "\nPulled: {} created, {} updated, {} deleted",
            pull_created, pull_updated, pull_deleted
        );
    }

    if push_created > 0 || push_updated > 0 || push_deleted > 0 {
        println!(
            "Pushed: {} created, {} updated, {} deleted",
            push_created, push_updated, push_deleted
        );
    }

    Ok(())
}
