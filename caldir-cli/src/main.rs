mod client;
mod commands;
mod render;

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

        /// Calendar to create event in (uses first calendar if not specified)
        #[arg(short, long)]
        calendar: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { provider } => commands::auth::run(&provider).await,
        Commands::Pull => commands::pull::run().await,
        Commands::Push => commands::push::run().await,
        Commands::Status => commands::status::run().await,
        Commands::New {
            title,
            start,
            calendar,
        } => commands::new::run(title, start, calendar).await,
    }
}
