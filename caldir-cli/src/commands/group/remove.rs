use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::group::Group;

pub fn run(calendar_slug: &str, group: &str) -> Result<()> {
    let group = Group::parse(group)?;
    let mut cal = Caldir::load()?.calendar(calendar_slug)?;
    let removed = cal.remove_from_group(&group)?;

    if removed {
        println!("Removed '{}' from group '{}'.", cal.slug, group);
    } else {
        println!("'{}' is not in group '{}'.", cal.slug, group);
    }

    Ok(())
}
