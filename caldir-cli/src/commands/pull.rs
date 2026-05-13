use anyhow::Result;
use caldir_core::{Caldir, CalendarDiff, Connection, DateRange, EventChange};
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::{resolve_sync_range, tui};

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
) -> Result<()> {
    let all_connections = caldir.connections();

    let connections = match calendar {
        Some(cal) => all_connections
            .into_iter()
            .filter(|conn| conn.as_ref().ok().and_then(|c| c.local().slug()) == Some(cal.as_str()))
            .collect(),
        None => all_connections,
    };

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

    let (created, updated, deleted) = applied.iter().fold((0, 0, 0), |(c, u, d), diff| {
        diff.incoming()
            .iter()
            .fold((c, u, d), |(c, u, d), change| match change {
                EventChange::Create(_) => (c + 1, u, d),
                EventChange::Update { .. } => (c, u + 1, d),
                EventChange::Delete(_) => (c, u, d + 1),
            })
    });

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
