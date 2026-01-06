use anyhow::Result;

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let calendars = caldir.calendars();

    for (i, cal) in calendars.iter().enumerate() {
        println!("{}", cal.render());

        let diff = cal.get_diff().await?;

        println!("{}", diff.render());

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    Ok(())
}
