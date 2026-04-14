use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::group::Group;
use owo_colors::OwoColorize;

pub fn run(group: &str) -> Result<()> {
    let group = Group::parse(group)?;

    let caldir = Caldir::load()?;
    let members = caldir.calendars_in_group(&group);

    if members.is_empty() {
        println!(
            "{}",
            format!("No calendars in group '{}'.", group).dimmed()
        );
        return Ok(());
    }

    for cal in members {
        println!("{}", cal.slug);
    }

    Ok(())
}
