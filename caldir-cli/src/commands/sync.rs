use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::{BatchDiff, CalendarDiff};
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(calendars: Vec<Calendar>, range: DateRange, verbose: bool) -> Result<()> {
    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = tui::create_spinner(cal.render());
        let result = CalendarDiff::from_calendar(cal, &range).await;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        match result {
            Ok(diff) => {
                println!("{}", diff.render_sync(verbose));
                diff.apply_pull()?;
                diff.apply_push().await?;
                diffs.push(diff);
            }
            Err(e) => println!("   {}", e.to_string().red()),
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
