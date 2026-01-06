use anyhow::Result;

use super::create_spinner;
use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = create_spinner(cal.render());
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
