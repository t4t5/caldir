mod caldir;
mod calendar;
mod commands;
mod config;
mod constants;
mod diff;
mod ics;
mod local;
mod provider;
mod remote;
mod utils;

use anyhow::Result;
use caldir::Caldir;
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
    Auth {
        provider: String, // e.g. "google"
    },
    Pull,
    Push,
    Status,
    New {
        title: String,

        /// Start date/time (e.g., "2025-03-20T15:00")
        #[arg(short, long)]
        start: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => commands::auth::run(&provider).await,
        Commands::Pull => {
            require_calendars()?;
            commands::pull::run().await
        }
        Commands::Push => {
            require_calendars()?;
            commands::push::run().await
        }
        Commands::Status => {
            require_calendars()?;
            commands::status::run().await
        }
        Commands::New { title, start } => {
            require_calendars()?;
            commands::new::run(title, start)
        }
    }
}

fn require_calendars() -> Result<()> {
    let caldir = Caldir::load()?;

    if caldir.calendars().is_empty() {
        anyhow::bail!(
            "No calendars found.\n\n\
            Connect your first calendar with:\n  \
            caldir-cli auth <provider>\n\n\
            Example:\n  \
            caldir-cli auth google"
        );
    }

    Ok(())
}
