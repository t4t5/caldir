mod commands;
mod render;
mod utils;

use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-cli")]
#[command(about = "Interact with your caldir events and sync to remote calendars")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Auth {
        provider: String, // e.g. "google"
    },
    Status {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Show events from this date (YYYY-MM-DD, or "start" for all past events)
        #[arg(long)]
        from: Option<String>,

        /// Show events until this date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
    Pull {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Pull events from this date (YYYY-MM-DD, or "start" for all past events)
        #[arg(long)]
        from: Option<String>,

        /// Pull events until this date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
    Push {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,
    },
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
        Commands::Status { calendar, from, to } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let range = DateRange::from_args(from.as_deref(), to.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::status::run(calendars, range).await
        }
        Commands::Pull { calendar, from, to } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let range = DateRange::from_args(from.as_deref(), to.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::pull::run(calendars, range).await
        }
        Commands::Push { calendar } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            commands::push::run(calendars).await
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

fn resolve_calendars(calendar_filter: Option<&str>) -> Result<Vec<Calendar>> {
    let caldir = Caldir::load()?;
    let all_calendars = caldir.calendars();

    match calendar_filter {
        Some(slug) => match all_calendars.into_iter().find(|c| c.slug == slug) {
            Some(cal) => Ok(vec![cal]),
            None => {
                let available: Vec<_> = caldir.calendars().iter().map(|c| c.slug.clone()).collect();
                anyhow::bail!(
                    "Calendar '{}' not found. Available: {}",
                    slug,
                    available.join(", ")
                );
            }
        },
        None => Ok(all_calendars),
    }
}
