use anyhow::Result;
use std::collections::HashMap;

use crate::provider::build_params;
use crate::{caldir, config, ics};

use super::{require_calendars, CalendarContext};

pub async fn run(force: bool) -> Result<()> {
    let cfg = config::load_config()?;
    require_calendars(&cfg)?;

    let mut total_stats = caldir::ApplyStats::default();

    for (calendar_name, calendar_config) in &cfg.calendars {
        // Skip if directory doesn't exist
        let calendar_dir = config::calendar_path(&cfg, calendar_name);
        if !calendar_dir.exists() {
            continue;
        }

        let ctx = CalendarContext::load(&cfg, calendar_name, calendar_config, false).await?;

        if !ctx.sync_diff.has_push_changes() {
            continue;
        }

        let stats = push_calendar(&ctx, force).await?;
        total_stats.add(&stats);
    }

    if total_stats.has_changes() {
        println!(
            "\nPushed {} created, {} updated, {} deleted",
            total_stats.created, total_stats.updated, total_stats.deleted
        );
    } else {
        println!("\nNo changes to push.");
    }

    Ok(())
}

async fn push_calendar(ctx: &CalendarContext, force: bool) -> Result<caldir::ApplyStats> {
    println!("\n📤 Pushing: {}", ctx.metadata.calendar_name);

    // Safety check: refuse to delete everything if local is empty (unless --force)
    if !ctx.sync_diff.to_push_delete.is_empty() && ctx.local_events.is_empty() && !force {
        anyhow::bail!(
            "Refusing to delete all {} events from remote (local calendar '{}' is empty).\n\
             If this is intentional, use: caldir-cli push --force",
            ctx.sync_diff.to_push_delete.len(),
            ctx.metadata.calendar_name
        );
    }

    let stats = apply_changes(ctx).await?;

    // Update sync state
    update_sync_state(&ctx.dir)?;

    Ok(stats)
}

async fn apply_changes(ctx: &CalendarContext) -> Result<caldir::ApplyStats> {
    let mut stats = caldir::ApplyStats::default();

    // Delete events from remote
    for change in &ctx.sync_diff.to_push_delete {
        println!("  Deleting: {}", change.uid);
        let params = build_params(
            &ctx.calendar_config.params,
            &[("event_id", serde_json::json!(change.uid))],
        );
        ctx.provider.delete_event(params).await?;
        stats.deleted += 1;
    }

    // Create new events on remote
    for change in &ctx.sync_diff.to_push_create {
        let Some(local) = find_local_by_filename(&ctx.local_events, &change.filename) else {
            continue;
        };

        println!("  Creating: {}", local.event.summary);
        let params = build_params(
            &ctx.calendar_config.params,
            &[("event", serde_json::to_value(&local.event)?)],
        );
        let created_event = ctx.provider.create_event(params).await?;

        // Write back with provider-assigned ID
        let new_content = ics::generate_ics(&created_event, &ctx.metadata)?;
        let base_filename = ics::generate_filename(&created_event);
        caldir::delete_event(&local.path)?;
        let new_filename = caldir::unique_filename(&base_filename, &ctx.dir, &created_event.id)?;
        caldir::write_event(&ctx.dir, &new_filename, &new_content)?;

        stats.created += 1;
    }

    // Update existing events on remote
    for change in &ctx.sync_diff.to_push_update {
        let Some(local) = find_local_by_filename(&ctx.local_events, &change.filename) else {
            continue;
        };

        println!("  Updating: {}", local.event.summary);
        let params = build_params(
            &ctx.calendar_config.params,
            &[("event", serde_json::to_value(&local.event)?)],
        );
        ctx.provider.update_event(params).await?;
        stats.updated += 1;
    }

    Ok(stats)
}

fn find_local_by_filename<'a>(
    local_events: &'a HashMap<String, caldir::LocalEvent>,
    filename: &str,
) -> Option<&'a caldir::LocalEvent> {
    local_events.values().find(|l| {
        l.path
            .file_name()
            .map(|f| f.to_string_lossy() == filename)
            .unwrap_or(false)
    })
}

fn update_sync_state(calendar_dir: &std::path::Path) -> Result<()> {
    let mut new_sync_state = config::SyncState::default();
    let local_events = caldir::read_all(calendar_dir)?;
    for uid in local_events.keys() {
        new_sync_state.synced_uids.insert(uid.clone());
    }
    config::save_sync_state(calendar_dir, &new_sync_state)
}
