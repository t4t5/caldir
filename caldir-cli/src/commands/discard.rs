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

    for (i, cal) in calendars.iter().enumerate() {
        if cal.remote().is_none() {
            println!("{}", cal.render());
            println!("   {}", "(local only)".dimmed());
        } else {
            let spinner = tui::create_spinner(cal.render());
            let result = CalendarDiff::from_calendar(cal, &range).await;
            spinner.finish_and_clear();
            println!("{}", cal.render());

            match result {
                Ok(diff) => {
                    println!("{}", diff.render_discard(verbose));
                    diffs.push(diff);
                }
                Err(e) => {
                    println!("   {}", e.to_string().red());
                }
            }
        }

        if i < calendars.len() - 1 {
            println!();
        }
    }

    let batch = BatchDiff(diffs);
    let total = batch.push_total();

    if total == 0 {
        return Ok(());
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
        "\nDiscarded {} {}",
        total,
        if total == 1 { "change" } else { "changes" }
    );

    Ok(())
}
