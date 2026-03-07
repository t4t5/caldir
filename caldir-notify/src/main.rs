mod check;
mod install;
mod notify;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-notify", about = "Desktop notifications for caldir reminders")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check for due reminders and fire notifications
    Check,
    /// Install the system timer/agent to run automatically
    Install,
    /// Uninstall the system timer/agent
    Uninstall,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Check => check::check_and_notify(),
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
