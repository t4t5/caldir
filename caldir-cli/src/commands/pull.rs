use anyhow::Result;
use owo_colors::OwoColorize;

use super::create_spinner;
use crate::caldir::Caldir;
use crate::diff_new::PullStats;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    let mut total = PullStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = create_spinner(cal.render());
        let result = cal.get_diff().await;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        match result {
            Ok(diff) => {
                println!("{}", diff.render_pull());

                let stats = diff.apply_pull()?;
                total.created += stats.created;
                total.updated += stats.updated;
                total.deleted += stats.deleted;
            }
            Err(e) => println!("   {}", e.to_string().red()),
        }

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    if total.created > 0 || total.updated > 0 || total.deleted > 0 {
        println!(
            "\nPulled {} created, {} updated, {} deleted",
            total.created, total.updated, total.deleted
        );
    }

    Ok(())
}
