use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    for (i, cal) in calendars.iter().enumerate() {
        // Show spinner while diff is loading:
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

        // Finished loading, show calendar + diff:
        println!("{}", cal.render());
        println!("{}", diff.render());

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    Ok(())
}
