mod caldir;
mod config;
mod diff;
mod event;
mod ics;
mod providers;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};

/// Number of days to sync in each direction (past and future)
const SYNC_DAYS: i64 = 365;

#[derive(Parser)]
#[command(name = "caldir-cli")]
#[command(about = "Interact with your local caldir directory and sync with external calendar providers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with a calendar provider
    Auth {
        /// Provider to authenticate with (default: google)
        #[arg(default_value = "google")]
        provider: String,
    },
    /// Pull events from cloud to local directory
    Pull,
    /// Push local changes to cloud calendars
    Push,
    /// Show changes between local directory and cloud calendars
    Status {
        /// Show which properties changed for each modified event
        #[arg(short, long)]
        verbose: bool,
    },
    /// Create a new local event
    New {
        /// Event title
        title: String,

        /// Start date/time (e.g., "2025-03-20" or "2025-03-20T15:00")
        #[arg(short, long)]
        start: String,

        /// End date/time
        #[arg(short, long, conflicts_with = "duration")]
        end: Option<String>,

        /// Duration (e.g., "30m", "1h", "2h30m")
        #[arg(short, long, conflicts_with = "end")]
        duration: Option<String>,

        /// Event description
        #[arg(long)]
        description: Option<String>,

        /// Event location
        #[arg(short, long)]
        location: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => cmd_auth(&provider).await,
        Commands::Pull => cmd_pull().await,
        Commands::Push => cmd_push().await,
        Commands::Status { verbose } => cmd_status(verbose).await,
        Commands::New {
            title,
            start,
            end,
            duration,
            description,
            location,
        } => cmd_new(title, start, end, duration, description, location).await,
    }
}

/// Refresh the access token if expired, saving the updated tokens.
/// Returns the (possibly refreshed) tokens.
async fn refresh_credentials_if_needed(
    google_config: &config::GoogleConfig,
    account_email: &str,
    tokens: config::AccountTokens,
    all_tokens: &mut config::Tokens,
) -> Result<config::AccountTokens> {
    if tokens
        .expires_at
        .map(|exp| exp < chrono::Utc::now())
        .unwrap_or(false)
    {
        println!("Access token expired for {}, refreshing...", account_email);
        let refreshed = providers::google::refresh_token(google_config, &tokens)
            .await
            .context("Failed to refresh token. Run 'caldir-cli auth' to re-authenticate.")?;
        all_tokens.google.insert(account_email.to_string(), refreshed.clone());
        config::save_tokens(all_tokens)?;
        Ok(refreshed)
    } else {
        Ok(tokens)
    }
}

async fn cmd_auth(provider: &str) -> Result<()> {
    match provider {
        "google" => {
            let mut cfg = config::load_config()?;
            let google_config = cfg
                .providers
                .google
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "Google Calendar not configured.\n\n\
                    Add to ~/.config/caldir/config.toml:\n\n\
                    [providers.google]\n\
                    client_id = \"your-client-id.apps.googleusercontent.com\"\n\
                    client_secret = \"your-client-secret\""
                ))?
                .clone();

            let tokens = providers::google::authenticate(&google_config).await?;

            // Discover the user's email from the authenticated account
            let email = providers::google::fetch_user_email(&google_config, &tokens).await?;
            println!("\nAuthenticated as: {}", email);

            // Fetch calendars and auto-add to config
            let calendars = providers::google::fetch_calendars(&google_config, &tokens).await?;
            println!("\nFound {} calendar(s):", calendars.len());

            for cal in &calendars {
                // Generate a slug name from calendar name
                let calendar_name = ics::slugify(&cal.name);
                let primary_marker = if cal.primary { " (primary)" } else { "" };
                println!("  Adding: {} ({}){}", calendar_name, cal.name, primary_marker);

                // Create calendar config
                let calendar_config = config::CalendarConfig {
                    provider: config::Provider::Google,
                    account: config::AccountEmail::from_string(email.clone()),
                    calendar_id: if cal.primary {
                        None // Primary calendar doesn't need explicit ID
                    } else {
                        Some(config::CalendarId::from_string(cal.id.clone()))
                    },
                };

                cfg.calendars.insert(calendar_name.clone(), calendar_config);

                // Set primary calendar as default
                if cal.primary && cfg.default_calendar.is_none() {
                    cfg.default_calendar = Some(calendar_name);
                }
            }

            // Save updated config
            config::save_config(&cfg)?;

            // Save tokens keyed by the discovered email
            let mut all_tokens = config::load_tokens()?;
            all_tokens.google.insert(email.clone(), tokens);
            config::save_tokens(&all_tokens)?;

            println!("\nCalendars added to config.toml.");
            println!("Run `caldir-cli pull` to sync your calendars.");

            Ok(())
        }
        _ => {
            anyhow::bail!("Unknown provider: {}. Supported: google", provider);
        }
    }
}

async fn cmd_pull() -> Result<()> {
    let cfg = config::load_config()?;
    let mut all_tokens = config::load_tokens()?;

    let google_config = cfg.providers.google.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.google.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-cli auth` first."
        );
    }

    // Get the calendar directory
    let calendar_dir = config::expand_path(&cfg.calendar_dir);
    std::fs::create_dir_all(&calendar_dir)?;

    let mut total_stats = caldir::ApplyStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    // Pull from all connected accounts
    let account_emails: Vec<String> = all_tokens.google.keys().cloned().collect();
    for account_email in account_emails {
        let tokens = all_tokens.google.get(&account_email).unwrap().clone();
        let account_tokens = refresh_credentials_if_needed(
            google_config,
            &account_email,
            tokens,
            &mut all_tokens,
        ).await?;

        println!("\nPulling from account: {}", account_email);

        // Fetch calendars to find the primary one
        let calendars =
            providers::google::fetch_calendars(google_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!("  Syncing calendar: {}", primary_calendar.name);

        // Fetch remote events
        let remote_events =
            providers::google::fetch_events(google_config, &account_tokens, &primary_calendar.id)
                .await?;
        println!("  Fetched {} events", remote_events.len());

        // Read local events
        let local_events = caldir::read_all(&calendar_dir)?;

        // Build calendar metadata for ICS generation
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
            source_url: Some(primary_calendar.source_url.clone()),
        };

        // Compute diff with time range awareness
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, false, time_range)?;

        // Apply changes
        let mut stats = caldir::ApplyStats {
            created: 0,
            updated: 0,
            deleted: 0,
        };

        // Create new events
        for change in &sync_diff.to_pull_create {
            // Find the event and generate ICS
            if let Some(event) = remote_events.iter().find(|e| {
                ics::generate_filename(e) == change.filename
            }) {
                let content = ics::generate_ics(event, &metadata)?;
                caldir::write_event(&calendar_dir, &change.filename, &content)?;
                stats.created += 1;
            }
        }

        // Update modified events
        for change in &sync_diff.to_pull_update {
            // Delete old file if it exists with different name
            if let Some(local) = local_events.values().find(|l| {
                l.path.file_name().map(|f| f.to_string_lossy().to_string())
                    != Some(change.filename.clone())
            }) {
                let _ = caldir::delete_event(&local.path);
            }
            // Find the event and generate ICS
            if let Some(event) = remote_events.iter().find(|e| {
                ics::generate_filename(e) == change.filename
            }) {
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

async fn cmd_push() -> Result<()> {
    let cfg = config::load_config()?;
    let mut all_tokens = config::load_tokens()?;

    let google_config = cfg.providers.google.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.google.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-cli auth` first."
        );
    }

    let calendar_dir = config::expand_path(&cfg.calendar_dir);

    let mut total_created = 0;
    let mut total_updated = 0;

    let account_emails: Vec<String> = all_tokens.google.keys().cloned().collect();
    for account_email in account_emails {
        let tokens = all_tokens.google.get(&account_email).unwrap().clone();
        let account_tokens = refresh_credentials_if_needed(
            google_config,
            &account_email,
            tokens,
            &mut all_tokens,
        ).await?;

        println!("\nPushing to account: {}", account_email);

        // Fetch calendars to find the primary one
        let calendars =
            providers::google::fetch_calendars(google_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!("  Syncing calendar: {}", primary_calendar.name);

        // Fetch remote events
        let remote_events =
            providers::google::fetch_events(google_config, &account_tokens, &primary_calendar.id)
                .await?;

        // Read local events
        let local_events = caldir::read_all(&calendar_dir)?;

        // Build calendar metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
            source_url: Some(primary_calendar.source_url.clone()),
        };

        // Compute diff with time range awareness
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, false, time_range)?;

        if sync_diff.to_push_create.is_empty() && sync_diff.to_push_update.is_empty() {
            println!("  No changes to push");
            continue;
        }

        // Push new local events
        for change in &sync_diff.to_push_create {
            // Find the local event by matching filename
            let local_event = local_events.values().find(|l| {
                l.path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    == Some(change.filename.clone())
            });

            if let Some(local) = local_event {
                // Parse local ICS to Event
                if let Some(event) = ics::parse_event(&local.content) {
                    println!("  Creating: {}", event.summary);

                    // Create event on Google Calendar and get back the full event
                    // with Google-assigned ID and Google-added fields (organizer, reminders, etc.)
                    let created_event = providers::google::create_event(
                        google_config,
                        &account_tokens,
                        &primary_calendar.id,
                        &event,
                    )
                    .await?;

                    // Generate new ICS content and filename from the Google-returned event
                    let new_content = ics::generate_ics(&created_event, &metadata)?;
                    let new_filename = ics::generate_filename(&created_event);

                    // Delete old file
                    caldir::delete_event(&local.path)?;

                    // Write new file with Google's event data
                    caldir::write_event(&calendar_dir, &new_filename, &new_content)?;

                    total_created += 1;
                } else {
                    eprintln!("  Warning: Could not parse {}", change.filename);
                }
            }
        }

        // Push updated events
        for change in &sync_diff.to_push_update {
            // Find the local event by matching filename
            let local_event = local_events.values().find(|l| {
                l.path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    == Some(change.filename.clone())
            });

            if let Some(local) = local_event {
                // Parse local ICS to Event
                if let Some(event) = ics::parse_event(&local.content) {
                    println!("  Updating: {}", event.summary);
                    providers::google::update_event(
                        google_config,
                        &account_tokens,
                        &primary_calendar.id,
                        &event,
                    )
                    .await?;
                    total_updated += 1;
                } else {
                    eprintln!("  Warning: Could not parse {}", change.filename);
                }
            }
        }
    }

    if total_created > 0 || total_updated > 0 {
        println!(
            "\nPushed {} created, {} updated",
            total_created, total_updated
        );
    } else {
        println!("\nNo changes to push.");
    }

    Ok(())
}

async fn cmd_status(verbose: bool) -> Result<()> {
    let cfg = config::load_config()?;
    let mut all_tokens = config::load_tokens()?;

    let google_config = cfg.providers.google.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.google.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-cli auth` first."
        );
    }

    let calendar_dir = config::expand_path(&cfg.calendar_dir);

    // Aggregate diffs from all accounts
    let mut all_to_pull_create: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_pull_update: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_pull_delete: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_push_create: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_push_update: Vec<diff::SyncChange> = Vec::new();

    let account_emails: Vec<String> = all_tokens.google.keys().cloned().collect();
    for account_email in account_emails {
        let tokens = all_tokens.google.get(&account_email).unwrap().clone();
        let account_tokens = refresh_credentials_if_needed(
            google_config,
            &account_email,
            tokens,
            &mut all_tokens,
        ).await?;

        // Fetch calendars to find the primary one
        let calendars =
            providers::google::fetch_calendars(google_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!(
            "Fetching from: {} ({})...",
            account_email, primary_calendar.name
        );

        // Fetch remote events
        let remote_events =
            providers::google::fetch_events(google_config, &account_tokens, &primary_calendar.id)
                .await?;

        // Read local events
        let local_events = caldir::read_all(&calendar_dir)?;

        // Build calendar metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
            source_url: Some(primary_calendar.source_url.clone()),
        };

        // Compute diff without applying (with time range awareness)
        let now = Utc::now();
        let time_range = Some((now - Duration::days(SYNC_DAYS), now + Duration::days(SYNC_DAYS)));
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, verbose, time_range)?;

        all_to_pull_create.extend(sync_diff.to_pull_create);
        all_to_pull_update.extend(sync_diff.to_pull_update);
        all_to_pull_delete.extend(sync_diff.to_pull_delete);
        all_to_push_create.extend(sync_diff.to_push_create);
        all_to_push_update.extend(sync_diff.to_push_update);
    }

    // Display results
    let has_pull_changes = !all_to_pull_create.is_empty()
        || !all_to_pull_update.is_empty()
        || !all_to_pull_delete.is_empty();
    let has_push_changes = !all_to_push_create.is_empty() || !all_to_push_update.is_empty();

    if !has_pull_changes && !has_push_changes {
        println!("\nEverything up to date.");
        return Ok(());
    }

    // Helper to print property changes
    let print_property_changes = |change: &diff::SyncChange| {
        if verbose && !change.property_changes.is_empty() {
            for prop_change in &change.property_changes {
                match (&prop_change.old_value, &prop_change.new_value) {
                    (Some(old), Some(new)) => {
                        println!("      {}: \"{}\" → \"{}\"", prop_change.property, old, new);
                    }
                    (Some(old), None) => {
                        println!("      {}: \"{}\" → (removed)", prop_change.property, old);
                    }
                    (None, Some(new)) => {
                        println!("      {}: (added) \"{}\"", prop_change.property, new);
                    }
                    (None, None) => {}
                }
            }
        }
    };

    // Display pull changes
    if has_pull_changes {
        println!("\nChanges to be pulled:\n");

        if !all_to_pull_create.is_empty() {
            println!("  New events ({}):", all_to_pull_create.len());
            for change in &all_to_pull_create {
                println!("    {}", change.filename);
            }
            println!();
        }

        if !all_to_pull_update.is_empty() {
            println!("  Modified events ({}):", all_to_pull_update.len());
            for change in &all_to_pull_update {
                println!("    {}", change.filename);
                print_property_changes(change);
            }
            println!();
        }

        if !all_to_pull_delete.is_empty() {
            println!("  Deleted events ({}):", all_to_pull_delete.len());
            for change in &all_to_pull_delete {
                println!("    {}", change.filename);
            }
            println!();
        }
    }

    // Display push changes
    if has_push_changes {
        println!("\nChanges to be pushed:\n");

        if !all_to_push_create.is_empty() {
            println!("  New events ({}):", all_to_push_create.len());
            for change in &all_to_push_create {
                println!("    {}", change.filename);
            }
            println!();
        }

        if !all_to_push_update.is_empty() {
            println!("  Modified events ({}):", all_to_push_update.len());
            for change in &all_to_push_update {
                println!("    {}", change.filename);
                print_property_changes(change);
            }
            println!();
        }
    }

    // Show appropriate action message
    if has_pull_changes && has_push_changes {
        println!("Run `caldir-cli pull` to pull changes, or `caldir-cli push` to push changes.");
    } else if has_pull_changes {
        println!("Run `caldir-cli pull` to apply these changes.");
    } else {
        println!("Run `caldir-cli push` to push these changes.");
    }

    Ok(())
}

async fn cmd_new(
    title: String,
    start: String,
    end: Option<String>,
    duration: Option<String>,
    description: Option<String>,
    location: Option<String>,
) -> Result<()> {
    use event::{Event, EventStatus, EventTime, Transparency};

    let cfg = config::load_config()?;
    let calendar_dir = config::expand_path(&cfg.calendar_dir);
    std::fs::create_dir_all(&calendar_dir)?;

    // Parse start time
    let start_time = ics::parse_cli_datetime(&start)?;

    // Calculate end time from --end, --duration, or default
    let end_time = if let Some(end_str) = end {
        ics::parse_cli_datetime(&end_str)?
    } else if let Some(dur_str) = duration {
        let dur = ics::parse_cli_duration(&dur_str)?;
        match &start_time {
            EventTime::DateTime(dt) => EventTime::DateTime(*dt + dur),
            EventTime::Date(d) => {
                // For all-day events with duration, add days
                let days = dur.num_days().max(1) as i64;
                EventTime::Date(*d + chrono::Duration::days(days))
            }
        }
    } else {
        // Default: 1 hour for timed events, same day for all-day
        match &start_time {
            EventTime::DateTime(dt) => EventTime::DateTime(*dt + chrono::Duration::hours(1)),
            EventTime::Date(d) => EventTime::Date(*d + chrono::Duration::days(1)),
        }
    };

    // Generate a unique local ID
    let event_id = format!("local-{}", uuid::Uuid::new_v4());

    // Create the event
    let event = Event {
        id: event_id,
        summary: title,
        description,
        location,
        start: start_time,
        end: end_time,
        status: EventStatus::Confirmed,
        recurrence: None,
        original_start: None,
        reminders: Vec::new(),
        transparency: Transparency::Opaque,
        organizer: None,
        attendees: Vec::new(),
        conference_url: None,
        updated: Some(chrono::Utc::now()),
        sequence: Some(0),
        custom_properties: vec![("X-CALDIR-ORIGIN".to_string(), "local".to_string())],
    };

    // Generate ICS content and filename
    let metadata = ics::CalendarMetadata {
        calendar_id: "local".to_string(),
        calendar_name: "Local Calendar".to_string(),
        source_url: None,
    };

    let ics_content = ics::generate_ics(&event, &metadata)?;
    let filename = ics::generate_filename(&event);

    // Write to disk
    caldir::write_event(&calendar_dir, &filename, &ics_content)?;

    println!("Created: {}", filename);

    Ok(())
}
