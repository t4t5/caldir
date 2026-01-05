use anyhow::Result;

use crate::caldir::Caldir;

pub async fn run() -> Result<()> {
    let caldir = Caldir::load()?;

    println!("Path: {:?}", caldir.data_path());
    println!("Calendars: {:?}", caldir.calendars().len());

    Ok(())
}
