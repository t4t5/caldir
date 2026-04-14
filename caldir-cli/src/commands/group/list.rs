use anyhow::Result;
use caldir_core::caldir::Caldir;
use owo_colors::OwoColorize;

pub fn run() -> Result<()> {
    let caldir = Caldir::load()?;
    let groups = caldir.all_groups();

    if groups.is_empty() {
        println!("{}", "No groups defined.".dimmed());
        return Ok(());
    }

    for group in groups {
        println!("{}", group);
    }

    Ok(())
}
