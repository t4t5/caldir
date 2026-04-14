use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::group::Group;

pub fn run(calendar_slug: &str, group: &str) -> Result<()> {
    let group = Group::parse(group)?;
    let mut cal = Caldir::load()?.calendar(calendar_slug)?;
    let added = cal.add_to_group(&group)?;

    if added {
        println!("Added '{}' to group '{}'.", cal.slug, group);
    } else {
        println!("'{}' is already in group '{}'.", cal.slug, group);
    }

    Ok(())
}
