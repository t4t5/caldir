pub mod auth;
pub mod new;
pub mod pull;
pub mod push;
pub mod status;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{Duration, Utc};

use crate::event::Event;
use crate::provider::{build_params, Provider};
use crate::{caldir, config, diff, ics};

/// Number of days to sync in each direction (past and future)
pub const SYNC_DAYS: i64 = 365;

/// Common context for calendar operations, loaded once per calendar.
pub struct CalendarContext {
    pub dir: PathBuf,
    pub local_events: HashMap<String, caldir::LocalEvent>,
    pub remote_events: Vec<Event>,
    pub sync_diff: diff::SyncDiff,
    pub metadata: ics::CalendarMetadata,
    pub provider: Provider,
    pub calendar_config: config::CalendarConfig,
}

impl CalendarContext {
    /// Load all context needed for sync operations on a single calendar.
    pub async fn load(
        cfg: &config::Config,
        calendar_name: &str,
        calendar_config: &config::CalendarConfig,
        verbose: bool,
    ) -> Result<Self> {
        let dir = config::calendar_path(cfg, calendar_name);

        // Read local events (empty if directory doesn't exist)
        let local_events = if dir.exists() {
            caldir::read_all(&dir)?
        } else {
            HashMap::new()
        };

        // Load sync state
        let sync_state = config::load_sync_state(&dir)?;

        // Fetch remote events
        let provider = Provider::new(&calendar_config.provider)?;
        let params = build_params(&calendar_config.params, &[]);
        let remote_events = provider.list_events(params).await?;

        // Compute diff
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff = diff::compute(
            &remote_events,
            &local_events,
            &dir,
            verbose,
            time_range,
            &sync_state.synced_uids,
        )?;

        // Build metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: get_calendar_id(&calendar_config.params, calendar_name),
            calendar_name: calendar_name.to_string(),
        };

        Ok(Self {
            dir,
            local_events,
            remote_events,
            sync_diff,
            metadata,
            provider,
            calendar_config: calendar_config.clone(),
        })
    }
}

/// Extract calendar_id from provider params, looking for {provider}_calendar_id
pub fn get_calendar_id(params: &HashMap<String, toml::Value>, fallback: &str) -> String {
    for (key, value) in params {
        if key.ends_with("_calendar_id")
            && let toml::Value::String(s) = value
        {
            return s.clone();
        }
    }
    fallback.to_string()
}

/// Shared error message for empty calendar config
pub fn require_calendars(cfg: &config::Config) -> Result<()> {
    if cfg.calendars.is_empty() {
        anyhow::bail!(
            "No calendars configured.\n\
            Run `caldir-cli auth <provider>` first, then add calendars to config.toml"
        );
    }
    Ok(())
}
