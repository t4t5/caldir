mod commands;
mod render;
mod utils;

#[cfg(test)]
mod test_utils;

use anyhow::Result;
use caldir_core::Caldir;
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
        /// Provider name (e.g. "google", "caldav", "icloud", "outlook")
        provider: Option<String>,

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

        /// Bypass safety checks (e.g. allow deleting all remote events when local is empty)
        #[arg(long)]
        force: bool,
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

        /// Bypass safety checks (e.g. allow deleting many remote events at once)
        #[arg(long)]
        force: bool,
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

        /// Reminder(s) before the event (e.g. "10m", "1h", "2 days"). Can be repeated.
        #[arg(short, long, conflicts_with = "no_reminders")]
        reminder: Vec<String>,

        /// Do not add any reminders (overrides default_reminders config)
        #[arg(long)]
        no_reminders: bool,
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
    // #[command(about = "List pending invites across calendars")]
    // Invites {
    //     /// Only show invites from this calendar (by slug)
    //     #[arg(short, long)]
    //     calendar: Option<String>,
    //
    //     /// Include already-responded invites (not just pending)
    //     #[arg(short, long)]
    //     all: bool,
    // },
    // #[command(about = "Respond to a calendar invites")]
    // Rsvp {
    //     /// Path to the .ics file (omit for interactive mode)
    //     path: Option<String>,
    //
    //     /// Response: accept, decline, maybe
    //     response: Option<String>,
    // },
    #[command(about = "Show configuration paths and calendar info")]
    Config,
    // #[command(about = "Update caldir and installed providers to the latest version")]
    // Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // `update` doesn't touch the caldir, so dispatch it before loading anything.
    // if let Commands::Update = cli.command {
    //     return commands::update::run().await;
    // }

    let mut caldir = Caldir::load()?;

    match cli.command {
        Commands::Connect { provider, hosted } => {
            commands::connect::run(&mut caldir, provider, hosted).await
        }
        Commands::Status {
            calendar,
            from,
            to,
            verbose,
        } => commands::status::run(&caldir, calendar, from, to, verbose).await,
        Commands::Pull {
            calendar,
            from,
            to,
            verbose,
        } => commands::pull::run(&caldir, calendar, from, to, verbose).await,
        Commands::Push {
            calendar,
            verbose,
            force,
        } => commands::push::run(&caldir, calendar, verbose, force).await,
        Commands::Sync {
            calendar,
            from,
            to,
            verbose,
            force,
        } => commands::sync::run(&caldir, calendar, from, to, verbose, force).await,
        Commands::Events { calendar, from, to } => {
            commands::events::run(&caldir, calendar, from, to)
        }
        Commands::Today { calendar } => commands::today::run(&caldir, calendar),
        Commands::Week { calendar } => commands::week::run(&caldir, calendar),
        Commands::New {
            title,
            start,
            end,
            duration,
            location,
            calendar,
            reminder,
            no_reminders,
        } => commands::new::run(
            &caldir,
            title,
            start,
            end,
            duration,
            location,
            calendar,
            reminder,
            no_reminders,
        ),
        Commands::Discard {
            calendar,
            verbose,
            force,
        } => commands::discard::run(&caldir, calendar, verbose, force).await,
        // Commands::Invites { calendar, all } => commands::invites::run(&caldir, calendar, all),
        // Commands::Rsvp { path, response } => commands::rsvp::run(&caldir, path, response),
        Commands::Config => commands::config::run(&caldir),
        // Commands::Update => unreachable!("handled above"),
    }
}
