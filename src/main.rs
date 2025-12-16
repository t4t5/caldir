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
    /// Show status of configured providers
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => cmd_auth(&provider).await,
        Commands::Pull => cmd_pull().await,
        Commands::Status => cmd_status().await,
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

async fn cmd_status() -> Result<()> {
    let config_path = config::config_path()?;
    let tokens_path = config::tokens_path()?;

    println!("caldir-sync status\n");

    // Config file
    if config_path.exists() {
        println!("Config: {}", config_path.display());
        match config::load_config() {
            Ok(cfg) => {
                if cfg.providers.gcal.is_some() {
                    println!("  - gcal: configured");
                }
            }
            Err(e) => {
                println!("  - Error loading config: {}", e);
            }
        }
    } else {
        println!("Config: not found (expected at {})", config_path.display());
    }

    println!();

    // Tokens / Connected accounts
    if tokens_path.exists() {
        println!("Tokens: {}", tokens_path.display());
        match config::load_tokens() {
            Ok(tokens) => {
                if tokens.gcal.is_empty() {
                    println!("  - gcal: no accounts connected");
                } else {
                    println!("  - gcal accounts:");
                    for (email, account_tokens) in &tokens.gcal {
                        let expired = account_tokens
                            .expires_at
                            .map(|exp| exp < chrono::Utc::now())
                            .unwrap_or(false);
                        let status = if expired { "expired" } else { "valid" };
                        println!("      {} ({})", email, status);
                    }
                }
            }
            Err(e) => {
                println!("  - Error loading tokens: {}", e);
            }
        }
    } else {
        println!("Tokens: no accounts connected");
    }

    Ok(())
}
