use anyhow::Result;
use caldir_core::{Caldir, CalendarDiff, Connection, DateRange};
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::{allow_mass_delete, connections, count_changes, tui};

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let calendar_slugs: Vec<String> = calendar.into_iter().collect();
    let connections = connections(caldir, &calendar_slugs);
    let range = DateRange::default();
    let mut applied: Vec<CalendarDiff> = Vec::new();
    let total = connections.len();

    for (i, connection) in connections.into_iter().enumerate() {
        match connection {
            Ok(mut connection) => {
                push_connection(
                    caldir,
                    &mut connection,
                    &range,
                    verbose,
                    force,
                    &mut applied,
                )
                .await;
            }
            Err(e) => println!("   {}", e.to_string().red()),
        }

        if i < total - 1 {
            println!();
        }
    }

    let (created, updated, deleted) = count_changes(applied.iter().flat_map(|d| d.outgoing()));

    if created > 0 || updated > 0 || deleted > 0 {
        println!(
            "\nPushed: {} created, {} updated, {} deleted",
            created, updated, deleted
        );
    }

    Ok(())
}

async fn push_connection(
    caldir: &Caldir,
    connection: &mut Connection,
    range: &DateRange,
    verbose: bool,
    force: bool,
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

    println!("{}", diff.render_push(verbose, caldir));

    if !allow_mass_delete(&diff, force) {
        return;
    }

    match connection.apply_outgoing_diff(&diff).await {
        Ok(()) => applied.push(diff),
        Err(e) => println!("   {}", e.to_string().red()),
    }
}
