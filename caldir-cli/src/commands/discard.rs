use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::{BatchDiff, CalendarDiff};
use dialoguer::Confirm;
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(calendars: Vec<Calendar>, verbose: bool, force: bool) -> Result<()> {
    let range = DateRange::default();

    let mut diffs = Vec::new();

    for cal in &calendars {
        let spinner = tui::create_spinner(cal.render());
        let result = CalendarDiff::from_calendar(cal, &range).await;
        spinner.finish_and_clear();

        match result {
            Ok(diff) => diffs.push(diff),
            Err(e) => {
                println!("{}", cal.render());
                println!("   {}", e.to_string().red());
            }
        }
    }

    let batch = BatchDiff(diffs);
    let (created, updated, deleted) = batch.push_counts();
    let total = created + updated + deleted;

    if total == 0 {
        println!("{}", "Nothing to discard".dimmed());
        return Ok(());
    }

    // Show what will be discarded
    for (i, diff) in batch.0.iter().enumerate() {
        if !diff.to_push.is_empty() {
            println!("{}", diff.calendar.render());
            println!("{}", diff.render_discard(verbose));

            if i < batch.0.len() - 1 {
                println!();
            }
        }
    }

    // Confirm unless --force
    if !force {
        println!();
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Discard {} {}?",
                total,
                if total == 1 { "change" } else { "changes" }
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            return Ok(());
        }
    }

    // Apply discard
    for diff in &batch.0 {
        if !diff.to_push.is_empty() {
            diff.apply_discard()?;
        }
    }

    println!(
        "\nDiscarded: {} created, {} updated, {} deleted",
        created, updated, deleted
    );

    Ok(())
}
