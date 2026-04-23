use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::{BatchDiff, CalendarDiff};
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(calendars: Vec<Calendar>, verbose: bool, force: bool) -> Result<()> {
    let range = DateRange::default();

    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = tui::create_spinner(cal.render());
        let result = CalendarDiff::from_calendar(cal, &range).await;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        match result {
            Ok(diff) => {
                println!("{}", diff.render_push(verbose));
                if !force && diff.would_wipe_remote()? {
                    println!(
                        "   {}",
                        format!(
                            "Refusing to delete all {} remote events for '{}' — local calendar is empty. \
                             If you deleted files by accident, run `caldir pull` to restore them. \
                             To proceed anyway, re-run with `--force`.",
                            diff.to_push.len(),
                            cal.slug,
                        )
                        .red()
                    );
                    continue;
                }
                diff.apply_push().await?;
                diffs.push(diff);
            }
            Err(e) => println!("   {}", e.to_string().red()),
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
