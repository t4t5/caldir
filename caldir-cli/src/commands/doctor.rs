use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use caldir_core::{Caldir, CalendarEvent, EventInstanceId};
use owo_colors::OwoColorize;

use crate::render::diff::Render;
use crate::utils::require_calendars;

/// Check the caldir for bad on-disk data. For now it only looks for duplicate
/// files — multiple local files that resolve to one event identity (e.g. the
/// same recurring instance saved in two timezones), which sync can't reconcile.
pub fn run(caldir: &Caldir) -> Result<()> {
    require_calendars(caldir)?;

    let mut issue_count = 0;

    for calendar in caldir.calendars().into_iter().filter_map(Result::ok) {
        let groups = match calendar.events() {
            Ok(events) => duplicate_file_groups(&events),
            Err(e) => {
                println!("{}", calendar.render(caldir));
                println!("   {}", e.to_string().red());
                continue;
            }
        };

        if groups.is_empty() {
            continue;
        }

        println!("{}", calendar.render(caldir));
        for paths in &groups {
            issue_count += 1;
            print_duplicate_group(paths);
        }
        println!();
    }

    if issue_count == 0 {
        println!("{} No problems found.", "✓".green());
    } else {
        println!(
            "{} {} duplicate {} found — delete all but one file in each.",
            "⚠".yellow(),
            issue_count,
            if issue_count == 1 { "group" } else { "groups" }
        );
    }

    Ok(())
}

/// Files grouped by event identity, keeping only the groups with more than one
/// file. Sorted for deterministic output.
fn duplicate_file_groups(events: &[CalendarEvent]) -> Vec<Vec<PathBuf>> {
    let mut by_id: HashMap<EventInstanceId, Vec<PathBuf>> = HashMap::new();
    for ce in events {
        by_id
            .entry(ce.event().event_instance_id())
            .or_default()
            .push(ce.path().to_path_buf());
    }

    let mut groups: Vec<Vec<PathBuf>> = by_id
        .into_values()
        .filter(|paths| paths.len() > 1)
        .collect();

    for paths in &mut groups {
        paths.sort();
    }
    groups.sort();
    groups
}

fn print_duplicate_group(paths: &[PathBuf]) {
    println!("   {} same event saved as multiple files:", "⚠".yellow());
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        println!("       {}", name.dimmed());
    }
}
