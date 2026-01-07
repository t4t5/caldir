mod commands;
mod config;
mod diff_new;
mod ics;
mod provider;
mod remote;
mod store;
mod sync;
mod utils;

mod caldir;
mod calendar;
mod local_event;

// Re-export caldir_core types as crate::event for internal use
pub use caldir_core as event;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-cli")]
#[command(about = "Interact with your caldir directory and sync to remote calendars")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with a calendar provider
    // Auth {
    //     /// Provider to authenticate with (e.g., "google")
    //     provider: String,
    // },
    /// Pull events from remote to local directory
    Pull,
    /// Push local changes to remote calendars
    Push,
    /// Show changes between local directory and remote calendars
    Status,
    // Create a new local event
    // New {
    //     /// Event title
    //     title: String,
    //
    //     /// Start date/time (e.g., "2025-03-20" or "2025-03-20T15:00")
    //     #[arg(short, long)]
    //     start: String,
    //
    //     /// End date/time
    //     #[arg(short, long, conflicts_with = "duration")]
    //     end: Option<String>,
    //
    //     /// Duration (e.g., "30m", "1h", "2h30m")
    //     #[arg(short, long, conflicts_with = "end")]
    //     duration: Option<String>,
    //
    //     /// Event description
    //     #[arg(long)]
    //     description: Option<String>,
    //
    //     /// Event location
    //     #[arg(short, long)]
    //     location: Option<String>,
    //
    //     /// Calendar to create the event in (defaults to default_calendar from config)
    //     #[arg(short, long)]
    //     calendar: Option<String>,
    // },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Commands::Auth { provider } => commands::auth::run(&provider).await,
        Commands::Pull => commands::pull::run().await,
        Commands::Push => commands::push::run().await,
        Commands::Status => commands::status::run().await,
        // Commands::New {
        //     title,
        //     start,
        //     end,
        //     duration,
        //     description,
        //     location,
        //     calendar,
        // } => commands::new::run(title, start, end, duration, description, location, calendar).await,
    }
}
