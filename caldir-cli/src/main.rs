mod commands;
mod render;
mod utils;

use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use chrono::{Datelike, Local, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-cli")]
#[command(version)]
#[command(about = "Interact with your caldir events and sync to remote calendars")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Connect to a remote calendar provider (e.g., Google Calendar)")]
    Connect {
        provider: String, // e.g. "google"

        /// Use hosted OAuth via caldir.org (default: true). Pass --hosted=false to use your own credentials.
        #[arg(long, default_value_t = true)]
        hosted: bool,
    },
    #[command(about = "Check if any events have changed (local and remote)")]
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

        /// Show all events (instead of compact view when >5 events)
        #[arg(short, long)]
        verbose: bool,
    },
    #[command(about = "Pull changes from remote calendars into local caldir")]
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

        /// Show all events (instead of compact view when >5 events)
        #[arg(short, long)]
        verbose: bool,
    },
    #[command(about = "Push changes from local caldir to remote calendars")]
    Push {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Show all events (instead of compact view when >5 events)
        #[arg(short, long)]
        verbose: bool,
    },
    #[command(about = "Sync changes between caldir and remote calendars (push + pull)")]
    Sync {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Sync events from this date (YYYY-MM-DD, or "start" for all past events)
        #[arg(long)]
        from: Option<String>,

        /// Sync events until this date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Show all events (instead of compact view when >5 events)
        #[arg(short, long)]
        verbose: bool,
    },
    #[command(about = "List upcoming events across all calendars")]
    Events {
        /// Only show events from this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Show events from this date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// Show events until this date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
    #[command(about = "Show today's events")]
    Today {
        /// Only show events from this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,
    },
    #[command(about = "Show this week's events (through Sunday)")]
    Week {
        /// Only show events from this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,
    },
    #[command(about = "Create a new event in caldir")]
    New {
        /// Event title
        title: Option<String>,

        /// Start date/time (natural language, e.g. "tomorrow 6pm")
        #[arg(short, long)]
        start: Option<String>,

        /// End date/time (natural language)
        #[arg(short, long)]
        end: Option<String>,

        /// Duration (e.g. "30m", "2 hours")
        #[arg(short, long)]
        duration: Option<String>,

        /// Location
        #[arg(short, long)]
        location: Option<String>,

        /// Calendar slug (defaults to default_calendar from config)
        #[arg(short = 'C', long)]
        calendar: Option<String>,
    },
    #[command(about = "Discard unpushed local changes (restore to remote state)")]
    Discard {
        /// Only operate on this calendar (by slug)
        #[arg(short, long)]
        calendar: Option<String>,

        /// Show all events (instead of compact view when >5 events)
        #[arg(short, long)]
        verbose: bool,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Show configuration paths and calendar info")]
    Config,
    #[command(about = "Update caldir and installed providers to the latest version")]
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Connect { provider, hosted } => commands::connect::run(&provider, hosted).await,
        Commands::Status {
            calendar,
            from,
            to,
            verbose,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let range = DateRange::from_args(from.as_deref(), to.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::status::run(calendars, range, verbose).await
        }
        Commands::Pull {
            calendar,
            from,
            to,
            verbose,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let range = DateRange::from_args(from.as_deref(), to.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::pull::run(calendars, range, verbose).await
        }
        Commands::Push { calendar, verbose } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            commands::push::run(calendars, verbose).await
        }
        Commands::Sync {
            calendar,
            from,
            to,
            verbose,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let range = DateRange::from_args(from.as_deref(), to.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::sync::run(calendars, range, verbose).await
        }
        Commands::Events {
            calendar,
            from,
            to,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            use caldir_core::date_range::{parse_date_start, parse_date_end};
            // Only parse dates if explicitly provided; events command has its own defaults
            let from_dt = from
                .as_deref()
                .map(parse_date_start)
                .transpose()
                .map_err(|e| anyhow::anyhow!(e))?;
            let to_dt = to
                .as_deref()
                .map(parse_date_end)
                .transpose()
                .map_err(|e| anyhow::anyhow!(e))?;
            commands::events::run(calendars, from_dt, to_dt)
        }
        Commands::Today { calendar } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let today = Local::now().date_naive();
            let start_of_today = today
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .with_timezone(&Utc);
            let end_of_today = today
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .with_timezone(&Utc);
            commands::events::run(calendars, Some(start_of_today), Some(end_of_today))
        }
        Commands::Week { calendar } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            let today = Local::now().date_naive();
            let start_of_today = today
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .with_timezone(&Utc);
            // num_days_from_monday(): Mon=0, Tue=1, ..., Sun=6
            let days_until_sunday = (6 - today.weekday().num_days_from_monday()) % 7;
            // If today is Sunday, show through next Sunday
            let days_until_sunday = if days_until_sunday == 0 { 7 } else { days_until_sunday };
            let end_of_sunday = (today + chrono::Duration::days(days_until_sunday as i64))
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .with_timezone(&Utc);
            commands::events::run(calendars, Some(start_of_today), Some(end_of_sunday))
        }
        Commands::New {
            title,
            start,
            end,
            duration,
            location,
            calendar,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(None)?;
            commands::new::run(title, start, end, duration, location, calendar, calendars)
        }
        Commands::Discard {
            calendar,
            verbose,
            force,
        } => {
            require_calendars()?;
            let calendars = resolve_calendars(calendar.as_deref())?;
            commands::discard::run(calendars, verbose, force).await
        }
        Commands::Config => commands::config::run(),
        Commands::Update => commands::update::run().await,
    }
}

fn require_calendars() -> Result<()> {
    let caldir = Caldir::load()?;

    if caldir.calendars().is_empty() {
        anyhow::bail!(
            "No calendars found.\n\n\
            Connect your first calendar with:\n  \
            caldir connect <provider>\n\n\
            Example:\n  \
            caldir connect google"
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
