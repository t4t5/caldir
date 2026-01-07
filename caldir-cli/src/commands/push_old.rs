use anyhow::Result;

use crate::provider::build_params;
use crate::{config, store, sync};

use super::{CalendarContext, require_calendars};

pub async fn run(force: bool) -> Result<()> {
    let cfg = config::load_config()?;
    require_calendars(&cfg)?;

    let mut total_stats = sync::ApplyStats::default();

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

async fn push_calendar(ctx: &CalendarContext, force: bool) -> Result<sync::ApplyStats> {
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

async fn apply_changes(ctx: &CalendarContext) -> Result<sync::ApplyStats> {
    let mut stats = sync::ApplyStats::default();

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
        let Some(local_event) = ctx.local_events.get(&change.uid) else {
            continue;
        };

        println!("  Creating: {}", local_event.event.summary);
        let params = build_params(
            &ctx.calendar_config.params,
            &[("event", serde_json::to_value(&local_event.event)?)],
        );
        let created_event = ctx.provider.create_event(params).await?;

        // Write back with provider-assigned ID
        store::update(&ctx.dir, local_event, &created_event, &ctx.metadata)?;

        stats.created += 1;
    }

    // Update existing events on remote
    for change in &ctx.sync_diff.to_push_update {
        let Some(local_event) = ctx.local_events.get(&change.uid) else {
            continue;
        };

        println!("  Updating: {}", local_event.event.summary);
        let params = build_params(
            &ctx.calendar_config.params,
            &[("event", serde_json::to_value(&local_event.event)?)],
        );
        ctx.provider.update_event(params).await?;
        stats.updated += 1;
    }

    Ok(stats)
}

fn update_sync_state(calendar_dir: &std::path::Path) -> Result<()> {
    let mut new_sync_state = sync::SyncState::default();
    let local_events = store::list(calendar_dir)?;
    for uid in local_events.keys() {
        new_sync_state.synced_uids.insert(uid.clone());
    }
    sync::save_state(calendar_dir, &new_sync_state)
}
