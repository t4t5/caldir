mod caldir;
mod config;
mod diff;
mod event;
mod ics;
mod providers;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-sync")]
#[command(about = "Sync cloud calendars to a local directory of .ics files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with a calendar provider
    Auth {
        /// Provider to authenticate with (default: gcal)
        #[arg(default_value = "gcal")]
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => cmd_auth(&provider).await,
        Commands::Pull => cmd_pull().await,
        Commands::Push => cmd_push().await,
        Commands::Status { verbose } => cmd_status(verbose).await,
    }
}

async fn cmd_auth(provider: &str) -> Result<()> {
    match provider {
        "gcal" => {
            let cfg = config::load_config()?;
            let gcal_config = cfg
                .providers
                .gcal
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "Google Calendar not configured.\n\n\
                    Add to ~/.config/caldir/config.toml:\n\n\
                    [providers.gcal]\n\
                    client_id = \"your-client-id.apps.googleusercontent.com\"\n\
                    client_secret = \"your-client-secret\""
                ))?;

            let tokens = providers::gcal::authenticate(gcal_config).await?;

            // Discover the user's email from the authenticated account
            let email = providers::gcal::fetch_user_email(gcal_config, &tokens).await?;
            println!("\nAuthenticated as: {}", email);

            // Fetch and display calendars
            let calendars = providers::gcal::fetch_calendars(gcal_config, &tokens).await?;
            println!("\nAvailable calendars:");
            for cal in &calendars {
                let primary_marker = if cal.primary { " (primary)" } else { "" };
                println!("  - {}{}", cal.name, primary_marker);
            }

            // Save tokens keyed by the discovered email
            let mut all_tokens = config::load_tokens()?;
            all_tokens.gcal.insert(email.clone(), tokens);
            config::save_tokens(&all_tokens)?;

            println!("\nTokens saved for account: {}", email);
            println!("You can now run `caldir-sync pull` to sync your calendar.");

            Ok(())
        }
        _ => {
            anyhow::bail!("Unknown provider: {}. Supported: gcal", provider);
        }
    }
}

async fn cmd_pull() -> Result<()> {
    let cfg = config::load_config()?;
    let mut all_tokens = config::load_tokens()?;

    let gcal_config = cfg.providers.gcal.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.gcal.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-sync auth` first."
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
    let account_emails: Vec<String> = all_tokens.gcal.keys().cloned().collect();
    for account_email in account_emails {
        let mut account_tokens = all_tokens.gcal.get(&account_email).unwrap().clone();

        // Check if token needs refresh
        if account_tokens
            .expires_at
            .map(|exp| exp < chrono::Utc::now())
            .unwrap_or(false)
        {
            println!("Access token expired for {}, refreshing...", account_email);
            account_tokens =
                providers::gcal::refresh_token(gcal_config, &account_tokens).await?;
            all_tokens
                .gcal
                .insert(account_email.clone(), account_tokens.clone());
            config::save_tokens(&all_tokens)?;
        }

        println!("\nPulling from account: {}", account_email);

        // Fetch calendars to find the primary one
        let calendars =
            providers::gcal::fetch_calendars(gcal_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!("  Syncing calendar: {}", primary_calendar.name);

        // Fetch remote events
        let remote_events =
            providers::gcal::fetch_events(gcal_config, &account_tokens, &primary_calendar.id)
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

        // Compute diff
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, false)?;

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

    let gcal_config = cfg.providers.gcal.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.gcal.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-sync auth` first."
        );
    }

    let calendar_dir = config::expand_path(&cfg.calendar_dir);

    let mut total_updated = 0;

    let account_emails: Vec<String> = all_tokens.gcal.keys().cloned().collect();
    for account_email in account_emails {
        let mut account_tokens = all_tokens.gcal.get(&account_email).unwrap().clone();

        // Check if token needs refresh
        if account_tokens
            .expires_at
            .map(|exp| exp < chrono::Utc::now())
            .unwrap_or(false)
        {
            println!("Access token expired for {}, refreshing...", account_email);
            account_tokens =
                providers::gcal::refresh_token(gcal_config, &account_tokens).await?;
            all_tokens
                .gcal
                .insert(account_email.clone(), account_tokens.clone());
            config::save_tokens(&all_tokens)?;
        }

        println!("\nPushing to account: {}", account_email);

        // Fetch calendars to find the primary one
        let calendars =
            providers::gcal::fetch_calendars(gcal_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!("  Syncing calendar: {}", primary_calendar.name);

        // Fetch remote events
        let remote_events =
            providers::gcal::fetch_events(gcal_config, &account_tokens, &primary_calendar.id)
                .await?;

        // Read local events
        let local_events = caldir::read_all(&calendar_dir)?;

        // Build calendar metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
            source_url: Some(primary_calendar.source_url.clone()),
        };

        // Compute diff
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, false)?;

        if sync_diff.to_push_update.is_empty() {
            println!("  No changes to push");
            continue;
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
                    providers::gcal::update_event(
                        gcal_config,
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

    if total_updated > 0 {
        println!("\nPushed {} event(s)", total_updated);
    } else {
        println!("\nNo changes to push.");
    }

    Ok(())
}

async fn cmd_status(verbose: bool) -> Result<()> {
    let cfg = config::load_config()?;
    let mut all_tokens = config::load_tokens()?;

    let gcal_config = cfg.providers.gcal.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Google Calendar not configured in config.toml")
    })?;

    if all_tokens.gcal.is_empty() {
        anyhow::bail!(
            "Not authenticated with Google Calendar.\n\
            Run `caldir-sync auth` first."
        );
    }

    let calendar_dir = config::expand_path(&cfg.calendar_dir);

    // Aggregate diffs from all accounts
    let mut all_to_pull_create: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_pull_update: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_pull_delete: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_push_create: Vec<diff::SyncChange> = Vec::new();
    let mut all_to_push_update: Vec<diff::SyncChange> = Vec::new();

    let account_emails: Vec<String> = all_tokens.gcal.keys().cloned().collect();
    for account_email in account_emails {
        let mut account_tokens = all_tokens.gcal.get(&account_email).unwrap().clone();

        // Refresh token if needed
        if account_tokens
            .expires_at
            .map(|exp| exp < chrono::Utc::now())
            .unwrap_or(false)
        {
            account_tokens =
                providers::gcal::refresh_token(gcal_config, &account_tokens).await?;
            all_tokens
                .gcal
                .insert(account_email.clone(), account_tokens.clone());
            config::save_tokens(&all_tokens)?;
        }

        // Fetch calendars to find the primary one
        let calendars =
            providers::gcal::fetch_calendars(gcal_config, &account_tokens).await?;
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
            providers::gcal::fetch_events(gcal_config, &account_tokens, &primary_calendar.id)
                .await?;

        // Read local events
        let local_events = caldir::read_all(&calendar_dir)?;

        // Build calendar metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
            source_url: Some(primary_calendar.source_url.clone()),
        };

        // Compute diff without applying
        let sync_diff =
            diff::compute(&remote_events, &local_events, &calendar_dir, &metadata, verbose)?;

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
        println!("Run `caldir-sync pull` to pull changes, or `caldir-sync push` to push changes.");
    } else if has_pull_changes {
        println!("Run `caldir-sync pull` to apply these changes.");
    } else {
        println!("Run `caldir-sync push` to push these changes.");
    }

    Ok(())
}
