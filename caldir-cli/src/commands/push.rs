use anyhow::Result;
use owo_colors::OwoColorize;

use caldir_lib::diff::BatchDiff;
use caldir_lib::Caldir;

use crate::render;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    let mut diffs = Vec::new();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = render::create_spinner(render::render_calendar(cal));
        let result = cal.get_diff().await;
        spinner.finish_and_clear();

        println!("{}", render::render_calendar(cal));

        match result {
            Ok(diff) => {
                println!("{}", render::render_push_diff(&diff));
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
