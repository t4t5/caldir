use anyhow::Result;
use owo_colors::OwoColorize;

use crate::caldir::Caldir;
use crate::diff::BatchDiff;
use crate::utils::tui;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = tui::create_spinner(cal.render());
        let result = cal.get_diff().await;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        match result {
            Ok(diff) => {
                println!("{}", diff.render_pull());
                diff.apply_pull()?;
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
    let (created, updated, deleted) = batch.pull_counts();

    if created > 0 || updated > 0 || deleted > 0 {
        println!(
            "\nPulled: {} created, {} updated, {} deleted",
            created, updated, deleted
        );
    }

    Ok(())
}
