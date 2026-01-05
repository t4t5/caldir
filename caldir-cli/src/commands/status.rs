use std::path::PathBuf;

use anyhow::Result;

use crate::{config, diff};

use super::{CalendarContext, require_calendars};

struct Provider(String);

struct Calendar {
    path: PathBuf,
    provider: Provider,
}

pub async fn run(verbose: bool) -> Result<()> {
    let cfg = config::load_config()?;
    require_calendars(&cfg)?;

    let mut any_changes = false;

    for (calendar_name, calendar_config) in &cfg.calendars {
        let ctx = CalendarContext::load(&cfg, calendar_name, calendar_config, verbose).await?;

        if !ctx.sync_diff.has_pull_changes() && !ctx.sync_diff.has_push_changes() {
            continue;
        }

        any_changes = true;
        print_diff(&ctx.metadata.calendar_name, &ctx.sync_diff, verbose);
    }

    if !any_changes {
        println!("Everything up to date.");
    } else {
        println!("\nRun `caldir-cli pull` to pull changes, or `caldir-cli push` to push changes.");
    }

    Ok(())
}

fn print_diff(calendar_name: &str, sync_diff: &diff::SyncDiff, verbose: bool) {
    println!("\n📅 {}", calendar_name);

    if sync_diff.has_pull_changes() {
        println!("  To pull:");
        for change in &sync_diff.to_pull_create {
            println!("    + {}", change.filename);
        }
        for change in &sync_diff.to_pull_update {
            println!("    ~ {}", change.filename);
            print_property_changes(change, verbose);
        }
        for change in &sync_diff.to_pull_delete {
            println!("    - {}", change.filename);
        }
    }

    if sync_diff.has_push_changes() {
        println!("  To push:");
        for change in &sync_diff.to_push_create {
            println!("    + {}", change.filename);
        }
        for change in &sync_diff.to_push_update {
            println!("    ~ {}", change.filename);
            print_property_changes(change, verbose);
        }
        for change in &sync_diff.to_push_delete {
            println!("    - {} (delete from remote)", change.uid);
        }
    }
}

fn print_property_changes(change: &diff::SyncChange, verbose: bool) {
    if !verbose || change.property_changes.is_empty() {
        return;
    }

    for prop_change in &change.property_changes {
        match (&prop_change.old_value, &prop_change.new_value) {
            (Some(old), Some(new)) => {
                println!(
                    "        {}: \"{}\" → \"{}\"",
                    prop_change.property, old, new
                );
            }
            (Some(old), None) => {
                println!("        {}: \"{}\" → (removed)", prop_change.property, old);
            }
            (None, Some(new)) => {
                println!("        {}: (added) \"{}\"", prop_change.property, new);
            }
            (None, None) => {}
        }
    }
}
