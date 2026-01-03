use anyhow::Result;
use chrono::{Duration, Utc};

use crate::provider::{build_params, Provider};
use crate::{caldir, config, diff, ics};

use super::{get_calendar_id, SYNC_DAYS};

pub async fn run(force: bool) -> Result<()> {
    let cfg = config::load_config()?;

    if cfg.calendars.is_empty() {
        anyhow::bail!(
            "No calendars configured.\n\
            Run `caldir-cli auth <provider>` first, then add calendars to config.toml"
        );
    }

    let mut total_created = 0;
    let mut total_updated = 0;
    let mut total_deleted = 0;

    // Push to each configured calendar
    for (calendar_name, calendar_config) in &cfg.calendars {
        let provider = Provider::new(&calendar_config.provider)?;

        // Get calendar-specific directory
        let calendar_dir = config::calendar_path(&cfg, calendar_name);
        if !calendar_dir.exists() {
            continue;
        }

        // Read local events from this calendar's directory
        let local_events = caldir::read_all(&calendar_dir)?;

        // Load sync state
        let sync_state = config::load_sync_state(&calendar_dir)?;

        // Fetch remote events
        let params = build_params(&calendar_config.params, &[]);
        let remote_events = provider.list_events(params).await?;

        // Build calendar metadata
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

        if !sync_diff.has_push_changes() {
            continue;
        }

        println!("\n📤 Pushing: {}", calendar_name);

        // Safety check: refuse to delete everything if local is empty (unless --force)
        if !sync_diff.to_push_delete.is_empty() && local_events.is_empty() && !force {
            anyhow::bail!(
                "Refusing to delete all {} events from remote (local calendar '{}' is empty).\n\
                 If this is intentional, use: caldir-cli push --force",
                sync_diff.to_push_delete.len(),
                calendar_name
            );
        }

        // Delete events from remote that were deleted locally
        for change in &sync_diff.to_push_delete {
            println!("  Deleting: {}", change.uid);
            let params = build_params(
                &calendar_config.params,
                &[("event_id", serde_json::json!(change.uid))],
            );
            provider.delete_event(params).await?;
            total_deleted += 1;
        }

        // Push new local events
        for change in &sync_diff.to_push_create {
            let local_event = local_events.values().find(|l| {
                l.path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    == Some(change.filename.clone())
            });

            if let Some(local) = local_event {
                let event = &local.event;
                println!("  Creating: {}", event.summary);

                let params = build_params(
                    &calendar_config.params,
                    &[("event", serde_json::to_value(event)?)],
                );
                let created_event = provider.create_event(params).await?;

                // Write back with provider-assigned ID
                let new_content = ics::generate_ics(&created_event, &metadata)?;
                let new_filename = ics::generate_filename(&created_event);

                caldir::delete_event(&local.path)?;
                caldir::write_event(&calendar_dir, &new_filename, &new_content)?;

                total_created += 1;
            }
        }

        // Push updated events
        for change in &sync_diff.to_push_update {
            let local_event = local_events.values().find(|l| {
                l.path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    == Some(change.filename.clone())
            });

            if let Some(local) = local_event {
                let event = &local.event;
                println!("  Updating: {}", event.summary);
                let params = build_params(
                    &calendar_config.params,
                    &[("event", serde_json::to_value(event)?)],
                );
                provider.update_event(params).await?;
                total_updated += 1;
            }
        }

        // Update sync state
        let mut new_sync_state = config::SyncState::default();
        let updated_local_events = caldir::read_all(&calendar_dir)?;
        for uid in updated_local_events.keys() {
            new_sync_state.synced_uids.insert(uid.clone());
        }
        config::save_sync_state(&calendar_dir, &new_sync_state)?;
    }

    if total_created > 0 || total_updated > 0 || total_deleted > 0 {
        println!(
            "\nPushed {} created, {} updated, {} deleted",
            total_created, total_updated, total_deleted
        );
    } else {
        println!("\nNo changes to push.");
    }

    Ok(())
}
