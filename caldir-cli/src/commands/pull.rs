use anyhow::Result;

use crate::event::Event;
use crate::{caldir, config, ics};

use super::{require_calendars, CalendarContext};

pub async fn run() -> Result<()> {
    let cfg = config::load_config()?;
    require_calendars(&cfg)?;

    let mut total_stats = caldir::ApplyStats::default();

    for (calendar_name, calendar_config) in &cfg.calendars {
        let ctx = CalendarContext::load(&cfg, calendar_name, calendar_config, false).await?;
        let stats = pull_calendar(&ctx).await?;
        total_stats.add(&stats);
    }

    println!(
        "\nTotal: {} created, {} updated, {} deleted",
        total_stats.created, total_stats.updated, total_stats.deleted
    );

    Ok(())
}

async fn pull_calendar(ctx: &CalendarContext) -> Result<caldir::ApplyStats> {
    println!("\n📅 Pulling: {}", ctx.metadata.calendar_name);
    println!("  Fetched {} events", ctx.remote_events.len());

    // Ensure directory exists
    std::fs::create_dir_all(&ctx.dir)?;

    // Apply changes
    let stats = apply_changes(ctx)?;

    // Update sync state
    update_sync_state(ctx)?;

    println!(
        "  {} created, {} updated, {} deleted",
        stats.created, stats.updated, stats.deleted
    );

    Ok(stats)
}

fn apply_changes(ctx: &CalendarContext) -> Result<caldir::ApplyStats> {
    let mut stats = caldir::ApplyStats::default();

    // Create new events
    for change in &ctx.sync_diff.to_pull_create {
        if let Some(event) = find_remote_by_filename(&ctx.remote_events, &change.filename) {
            let content = ics::generate_ics(event, &ctx.metadata)?;
            caldir::write_event(&ctx.dir, &change.filename, &content)?;
            stats.created += 1;
        }
    }

    // Update modified events
    for change in &ctx.sync_diff.to_pull_update {
        // Delete old file if filename changed (find by UID, not by scanning all files)
        if let Some(local) = ctx.local_events.get(&change.uid) {
            let old_filename = local.path.file_name().map(|f| f.to_string_lossy().to_string());
            if old_filename != Some(change.filename.clone()) {
                let _ = caldir::delete_event(&local.path);
            }
        }

        if let Some(event) = find_remote_by_filename(&ctx.remote_events, &change.filename) {
            let content = ics::generate_ics(event, &ctx.metadata)?;
            caldir::write_event(&ctx.dir, &change.filename, &content)?;
            stats.updated += 1;
        }
    }

    // Delete removed events
    for change in &ctx.sync_diff.to_pull_delete {
        let path = ctx.dir.join(&change.filename);
        caldir::delete_event(&path)?;
        stats.deleted += 1;
    }

    Ok(stats)
}

fn find_remote_by_filename<'a>(remote_events: &'a [Event], filename: &str) -> Option<&'a Event> {
    remote_events
        .iter()
        .find(|e| ics::generate_filename(e) == filename)
}

fn update_sync_state(ctx: &CalendarContext) -> Result<()> {
    let mut new_sync_state = config::SyncState::default();

    // Start with existing local UIDs
    for uid in ctx.local_events.keys() {
        new_sync_state.synced_uids.insert(uid.clone());
    }

    // Add newly created
    for change in &ctx.sync_diff.to_pull_create {
        new_sync_state.synced_uids.insert(change.uid.clone());
    }

    // Remove deleted (both pull and push deletes)
    for change in &ctx.sync_diff.to_pull_delete {
        new_sync_state.synced_uids.remove(&change.uid);
    }
    for change in &ctx.sync_diff.to_push_delete {
        new_sync_state.synced_uids.remove(&change.uid);
    }

    config::save_sync_state(&ctx.dir, &new_sync_state)
}
