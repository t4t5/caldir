use anyhow::Result;
use caldir_core::{Caldir, CaldirError, Connection, DateRange};
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::tui;
use crate::utils::{require_calendars, resolve_sync_range};

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
) -> Result<()> {
    require_calendars(caldir)?;

    let all_connections = caldir.connections();

    let connections = match calendar {
        Some(cal) => all_connections
            .into_iter()
            .filter(|conn| conn.as_ref().ok().and_then(|c| c.local().slug()) == Some(cal.as_str()))
            .collect(),
        None => all_connections,
    };

    let range = resolve_sync_range(from, to)?;

    run_parsed(caldir, connections, range, verbose).await
}

async fn run_parsed(
    caldir: &Caldir,
    connections: Vec<Result<Connection, CaldirError>>,
    range: DateRange,
    verbose: bool,
) -> Result<()> {
    let total = connections.len();

    for (i, connection) in connections.into_iter().enumerate() {
        match connection {
            Ok(connection) => {
                let cal = connection.local();
                let spinner = tui::create_spinner(cal.render(caldir));
                let result = connection.diff(&range).await;
                spinner.finish_and_clear();

                println!("{}", cal.render(caldir));

                match result {
                    Ok(diff) => println!("{}", diff.render(verbose, caldir)),
                    Err(e) => println!("   {}", e.to_string().red()),
                }
            }
            Err(e) => {
                println!("   {}", e.to_string().red());
            }
        }

        // Add spacing between calendars (but not after the last one)
        if i < total - 1 {
            println!();
        }
    }

    Ok(())
}
