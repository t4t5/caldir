use anyhow::Result;
use caldir_core::{Caldir, CalendarDiff, Connection, DateRange};
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::{connections, count_changes, resolve_sync_range, tui};

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
) -> Result<()> {
    let connections = connections(caldir, calendar.as_deref());
    let range = resolve_sync_range(from, to)?;
    let mut applied: Vec<CalendarDiff> = Vec::new();
    let total = connections.len();

    for (i, connection) in connections.into_iter().enumerate() {
        match connection {
            Ok(mut connection) => {
                pull_connection(caldir, &mut connection, &range, verbose, &mut applied).await;
            }
            Err(e) => println!("   {}", e.to_string().red()),
        }

        if i < total - 1 {
            println!();
        }
    }

    let (created, updated, deleted) = count_changes(applied.iter().flat_map(|d| d.incoming()));

    if created > 0 || updated > 0 || deleted > 0 {
        println!(
            "\nPulled: {} created, {} updated, {} deleted",
            created, updated, deleted
        );
    }

    Ok(())
}

async fn pull_connection(
    caldir: &Caldir,
    connection: &mut Connection,
    range: &DateRange,
    verbose: bool,
    applied: &mut Vec<CalendarDiff>,
) {
    let header = connection.local().render(caldir);
    let spinner = tui::create_spinner(header.clone());
    let result = connection.diff(range).await;
    spinner.finish_and_clear();

    println!("{}", header);

    let diff = match result {
        Ok(diff) => diff,
        Err(e) => {
            println!("   {}", e.to_string().red());
            return;
        }
    };

    println!("{}", diff.render_pull(verbose, caldir));

    match connection.apply_incoming_diff(&diff) {
        Ok(()) => applied.push(diff),
        Err(e) => println!("   {}", e.to_string().red()),
    }
}
