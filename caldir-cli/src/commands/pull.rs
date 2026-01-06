use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::caldir::Caldir;
use crate::diff_new::PullStats;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();
    let mut total = PullStats { created: 0, updated: 0, deleted: 0 };

    for cal in &calendars {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["-", "\\", "|", "/"])
                .template("{msg} {spinner}")
                .unwrap(),
        );
        spinner.set_message(cal.render());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let diff = cal.get_diff().await?;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        if diff.to_pull.is_empty() {
            println!("   No changes to pull");
            continue;
        }

        println!("{}", diff.render_pull());

        let stats = diff.apply_pull()?;
        total.created += stats.created;
        total.updated += stats.updated;
        total.deleted += stats.deleted;
    }

    if total.created > 0 || total.updated > 0 || total.deleted > 0 {
        println!(
            "\nPulled {} created, {} updated, {} deleted",
            total.created, total.updated, total.deleted
        );
    }

    Ok(())
}
