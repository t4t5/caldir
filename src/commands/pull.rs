use anyhow::Result;
use chrono::{Duration, Utc};

use crate::provider::{build_params, Provider};
use crate::{caldir, config, diff, ics};

use super::{get_calendar_id, SYNC_DAYS};

pub async fn run() -> Result<()> {
    let cfg = config::load_config()?;

    if cfg.calendars.is_empty() {
        anyhow::bail!(
            "No calendars configured.\n\
            Run `caldir-cli auth <provider>` first, then add calendars to config.toml"
        );
    }

    let mut total_stats = caldir::ApplyStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    // Pull from each configured calendar
    for (calendar_name, calendar_config) in &cfg.calendars {
        let provider = Provider::new(&calendar_config.provider)?;

        println!("\n📅 Pulling: {}", calendar_name);

        // List remote events
        let params = build_params(&calendar_config.params, &[]);
        let remote_events = provider.list_events(params).await?;
        println!("  Fetched {} events", remote_events.len());

        // Get calendar-specific directory
        let calendar_dir = config::calendar_path(&cfg, calendar_name);
        std::fs::create_dir_all(&calendar_dir)?;

        // Read local events from this calendar's directory
        let local_events = caldir::read_all(&calendar_dir)?;

        // Load sync state to know which events have been synced before
        let sync_state = config::load_sync_state(&calendar_dir)?;

        // Build calendar metadata for ICS generation
        let metadata = ics::CalendarMetadata {
            calendar_id: get_calendar_id(&calendar_config.params, calendar_name),
            calendar_name: calendar_name.clone(),
        };

        // Compute diff with time range awareness and sync state
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff = diff::compute(
            &remote_events,
            &local_events,
            &calendar_dir,
            false,
            time_range,
            &sync_state.synced_uids,
        )?;

        // Apply changes
        let mut stats = caldir::ApplyStats {
            created: 0,
            updated: 0,
            deleted: 0,
        };

        // Create new events
        for change in &sync_diff.to_pull_create {
            if let Some(event) = remote_events
                .iter()
                .find(|e| ics::generate_filename(e) == change.filename)
            {
                let content = ics::generate_ics(event, &metadata)?;
                caldir::write_event(&calendar_dir, &change.filename, &content)?;
                stats.created += 1;
            }
        }

        // Update modified events
        for change in &sync_diff.to_pull_update {
            if let Some(local) = local_events.values().find(|l| {
                l.path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    != Some(change.filename.clone())
            }) {
                let _ = caldir::delete_event(&local.path);
            }
            if let Some(event) = remote_events
                .iter()
                .find(|e| ics::generate_filename(e) == change.filename)
            {
                let content = ics::generate_ics(event, &metadata)?;
                caldir::write_event(&calendar_dir, &change.filename, &content)?;
                stats.updated += 1;
            }
        }

        // Delete removed events
        for change in &sync_diff.to_pull_delete {
            let path = calendar_dir.join(&change.filename);
            caldir::delete_event(&path)?;
            stats.deleted += 1;
        }

        // Update sync state with current local UIDs
        let mut new_sync_state = config::SyncState::default();

        for uid in local_events.keys() {
            new_sync_state.synced_uids.insert(uid.clone());
        }
        for change in &sync_diff.to_pull_create {
            new_sync_state.synced_uids.insert(change.uid.clone());
        }
        for change in &sync_diff.to_pull_delete {
            new_sync_state.synced_uids.remove(&change.uid);
        }
        for change in &sync_diff.to_push_delete {
            new_sync_state.synced_uids.remove(&change.uid);
        }

        config::save_sync_state(&calendar_dir, &new_sync_state)?;

        total_stats.created += stats.created;
        total_stats.updated += stats.updated;
        total_stats.deleted += stats.deleted;

        println!(
            "  {} created, {} updated, {} deleted",
            stats.created, stats.updated, stats.deleted
        );
    }

    println!(
        "\nTotal: {} created, {} updated, {} deleted",
        total_stats.created, total_stats.updated, total_stats.deleted
    );

    Ok(())
}
