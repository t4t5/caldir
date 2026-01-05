use anyhow::Result;

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;

    for cal in caldir.calendars() {
        println!("Calendar: {}", cal.name);
        let diff = cal.get_diff().await?;
        print!("{}", diff);
    }

    Ok(())
}
