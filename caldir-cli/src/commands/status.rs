use anyhow::Result;
use owo_colors::OwoColorize;

use super::create_spinner;
use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = create_spinner(cal.render());
        let result = cal.get_diff().await;
        spinner.finish_and_clear();

        // Show calendar name
        println!("{}", cal.render());

        // Show diff or error
        match result {
            Ok(diff) => println!("{}", diff.render()),
            Err(e) => println!("   {}", e.to_string().red()),
        }

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    Ok(())
}
