use anyhow::Result;
use chrono::{Duration, Utc};

use crate::provider::{build_params, Provider};
use crate::{caldir, config, diff};

use super::SYNC_DAYS;

pub async fn run(verbose: bool) -> Result<()> {
    let cfg = config::load_config()?;

    if cfg.calendars.is_empty() {
        anyhow::bail!(
            "No calendars configured.\n\
            Run `caldir-cli auth <provider>` first, then add calendars to config.toml"
        );
    }

    // Helper to print property changes
    let print_property_changes = |change: &diff::SyncChange| {
        if verbose && !change.property_changes.is_empty() {
            for prop_change in &change.property_changes {
                match (&prop_change.old_value, &prop_change.new_value) {
                    (Some(old), Some(new)) => {
                        println!("        {}: \"{}\" → \"{}\"", prop_change.property, old, new);
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
    };

    let mut any_changes = false;

    // Check status for each configured calendar
    for (calendar_name, calendar_config) in &cfg.calendars {
        let provider = Provider::new(&calendar_config.provider)?;

        // Get calendar-specific directory
        let calendar_dir = config::calendar_path(&cfg, calendar_name);

        // Fetch remote events
        let params = build_params(&calendar_config.params, &[]);
        let remote_events = provider.list_events(params).await?;

        // Read local events (empty if directory doesn't exist)
        let local_events = if calendar_dir.exists() {
            caldir::read_all(&calendar_dir)?
        } else {
            std::collections::HashMap::new()
        };

        // Load sync state
        let sync_state = config::load_sync_state(&calendar_dir)?;

        // Compute diff with sync state
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff = diff::compute(
            &remote_events,
            &local_events,
            &calendar_dir,
            verbose,
            time_range,
            &sync_state.synced_uids,
        )?;

        if !sync_diff.has_pull_changes() && !sync_diff.has_push_changes() {
            continue;
        }

        any_changes = true;
        println!("\n📅 {}", calendar_name);

        // Display pull changes
        if sync_diff.has_pull_changes() {
            println!("  To pull:");
            for change in &sync_diff.to_pull_create {
                println!("    + {}", change.filename);
            }
            for change in &sync_diff.to_pull_update {
                println!("    ~ {}", change.filename);
                print_property_changes(change);
            }
            for change in &sync_diff.to_pull_delete {
                println!("    - {}", change.filename);
            }
        }

        // Display push changes
        if sync_diff.has_push_changes() {
            println!("  To push:");
            for change in &sync_diff.to_push_create {
                println!("    + {}", change.filename);
            }
            for change in &sync_diff.to_push_update {
                println!("    ~ {}", change.filename);
                print_property_changes(change);
            }
            for change in &sync_diff.to_push_delete {
                println!("    - {} (delete from remote)", change.uid);
            }
        }
    }

    if !any_changes {
        println!("Everything up to date.");
    } else {
        println!("\nRun `caldir-cli pull` to pull changes, or `caldir-cli push` to push changes.");
    }

    Ok(())
}
