use anyhow::Result;

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;

    for cal in caldir.calendars() {
        let events = cal.remote().list_events().await?;
        println!("Calendar: {} ({} remote events)", cal.name, events.len());
    }

    Ok(())
}
