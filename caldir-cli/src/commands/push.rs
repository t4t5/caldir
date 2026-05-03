use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::{BatchDiff, CalendarDiff};
use owo_colors::OwoColorize;

use crate::commands::guards::allow_mass_delete;
use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let settings = caldir.settings();
    let range = DateRange::default();

    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        if cal.remote().is_none() {
            println!("{}", cal.render(settings));
            println!("   {}", "(local only)".dimmed());
        } else {
            let spinner = tui::create_spinner(cal.render(settings));
            let result = CalendarDiff::from_calendar(caldir, cal, &range).await;
            spinner.finish_and_clear();

            println!("{}", cal.render(settings));

            match result {
                Ok(diff) => {
                    println!("{}", diff.render_push(verbose, settings));
                    if !allow_mass_delete(&diff, force) {
                        continue;
                    }
                    diff.apply_push().await?;
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
    let (created, updated, deleted) = batch.push_counts();

    if created > 0 || updated > 0 || deleted > 0 {
        println!(
            "\nPushed: {} created, {} updated, {} deleted",
            created, updated, deleted
        );
    }

    Ok(())
}
