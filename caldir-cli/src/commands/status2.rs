use anyhow::Result;

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;

    for cal in caldir.calendars() {
        let remote_events = cal.remote().list_events().await?;
        let local_events = cal.events()?;

        println!(
            "Calendar: {} ({} local events, {} remote events)",
            cal.name,
            local_events.len(),
            remote_events.len()
        );
    }

    Ok(())
}
