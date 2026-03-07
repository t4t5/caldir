mod check;
mod install;
mod notify;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "caldir-notify", about = "Desktop notifications for caldir reminders")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Install the system timer/agent to run automatically
    Install,
    /// Uninstall the system timer/agent
    Uninstall,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        None => check::check_and_notify(),
        Some(Command::Install) => install::install(),
        Some(Command::Uninstall) => install::uninstall(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
