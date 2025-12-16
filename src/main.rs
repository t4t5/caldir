mod config;
mod ics;
mod providers;
mod sync;

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
    /// Show changes between local directory and cloud calendars
    Status {
        /// Show detailed diff information for debugging
        #[arg(long)]
        debug: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => cmd_auth(&provider).await,
        Commands::Pull => cmd_pull().await,
        Commands::Status { debug } => cmd_status(debug).await,
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

    let mut total_stats = sync::SyncStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    // Pull from all connected accounts
    let account_emails: Vec<String> = all_tokens.gcal.keys().cloned().collect();
    for account_email in account_emails {
        let mut account_tokens = all_tokens.gcal.get(&account_email).unwrap().clone();

        // Check if token needs refresh
        if account_tokens.expires_at.map(|exp| exp < chrono::Utc::now()).unwrap_or(false) {
            println!("Access token expired for {}, refreshing...", account_email);
            account_tokens = providers::gcal::refresh_token(gcal_config, &account_tokens).await?;
            all_tokens.gcal.insert(account_email.clone(), account_tokens.clone());
            config::save_tokens(&all_tokens)?;
        }

        println!("\nPulling from account: {}", account_email);

        // Fetch calendars to find the primary one
        let calendars = providers::gcal::fetch_calendars(gcal_config, &account_tokens).await?;
        let primary_calendar = calendars
            .iter()
            .find(|c| c.primary)
            .or_else(|| calendars.first())
            .ok_or_else(|| anyhow::anyhow!("No calendars found for {}", account_email))?;

        println!("  Syncing calendar: {}", primary_calendar.name);

        // Fetch events
        let events = providers::gcal::fetch_events(gcal_config, &account_tokens, &primary_calendar.id).await?;
        println!("  Fetched {} events", events.len());

        // Build calendar metadata for ICS generation
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
        };

        // Sync to local directory
        let stats = sync::sync_events_to_dir(&events, &calendar_dir, &metadata)?;
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

async fn cmd_status(debug: bool) -> Result<()> {
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
    let mut all_to_create: Vec<sync::SyncChange> = Vec::new();
    let mut all_to_update: Vec<sync::SyncChange> = Vec::new();
    let mut all_to_delete: Vec<sync::SyncChange> = Vec::new();

    let account_emails: Vec<String> = all_tokens.gcal.keys().cloned().collect();
    for account_email in account_emails {
        let mut account_tokens = all_tokens.gcal.get(&account_email).unwrap().clone();

        // Refresh token if needed
        if account_tokens
            .expires_at
            .map(|exp| exp < chrono::Utc::now())
            .unwrap_or(false)
        {
            account_tokens = providers::gcal::refresh_token(gcal_config, &account_tokens).await?;
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

        // Fetch events
        let events =
            providers::gcal::fetch_events(gcal_config, &account_tokens, &primary_calendar.id)
                .await?;

        // Build calendar metadata
        let metadata = ics::CalendarMetadata {
            calendar_id: primary_calendar.id.clone(),
            calendar_name: primary_calendar.name.clone(),
        };

        // Compute diff without applying
        let diff = sync::compute_sync_diff(&events, &calendar_dir, &metadata, debug)?;

        all_to_create.extend(diff.to_create);
        all_to_update.extend(diff.to_update);
        all_to_delete.extend(diff.to_delete);
    }

    // Display results
    let has_changes =
        !all_to_create.is_empty() || !all_to_update.is_empty() || !all_to_delete.is_empty();

    if !has_changes {
        println!("\nEverything up to date.");
        return Ok(());
    }

    println!("\nChanges to be pulled:\n");

    if !all_to_create.is_empty() {
        println!("  New events ({}):", all_to_create.len());
        for change in &all_to_create {
            println!("    {}", change.filename);
        }
        println!();
    }

    if !all_to_update.is_empty() {
        println!("  Modified events ({}):", all_to_update.len());
        for change in &all_to_update {
            println!("    {}", change.filename);
        }
        println!();
    }

    if !all_to_delete.is_empty() {
        println!("  Deleted events ({}):", all_to_delete.len());
        for change in &all_to_delete {
            println!("    {}", change.filename);
        }
        println!();
    }

    println!("Run `caldir-sync pull` to apply these changes.");

    Ok(())
}
