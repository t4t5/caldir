use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::Calendar;
use caldir_core::DateRange;
use caldir_core::{BatchDiff, CalendarDiff};
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
) -> Result<()> {
    require_calendars(&caldir)?;
    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;
    let range =
        DateRange::from_args(from.as_deref(), to.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    run_parsed(&caldir, calendars, range, verbose).await
}

async fn run_parsed(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    range: DateRange,
    verbose: bool,
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
                    println!("{}", diff.render_pull(verbose, caldir));
                    diff.apply_pull()?;
                    diffs.push(diff);
                }
                Err(e) => println!("   {}", e.to_string().red()),
            }
        }

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    let batch = BatchDiff(diffs);
    let (created, updated, deleted) = batch.pull_counts();

    if created > 0 || updated > 0 || deleted > 0 {
        println!(
            "\nPulled: {} created, {} updated, {} deleted",
            created, updated, deleted
        );
    }

    Ok(())
}
