use anyhow::Result;
use caldir_core::{Caldir, Connection, DateRange};
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::{allow_mass_delete, connections, count_changes, resolve_sync_range, tui};

type Counts = (usize, usize, usize);

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let connections = connections(caldir, calendar.as_deref());
    let range = resolve_sync_range(from, to)?;
    let mut pulled: Counts = (0, 0, 0);
    let mut pushed: Counts = (0, 0, 0);
    let total = connections.len();

    for (i, connection) in connections.into_iter().enumerate() {
        match connection {
            Ok(mut connection) => {
                sync_connection(
                    caldir,
                    &mut connection,
                    &range,
                    verbose,
                    force,
                    &mut pulled,
                    &mut pushed,
                )
                .await;
            }
            Err(e) => println!("   {}", e.to_string().red()),
        }

        if i < total - 1 {
            println!();
        }
    }

    if pulled != (0, 0, 0) || pushed != (0, 0, 0) {
        println!();
    }

    if pulled != (0, 0, 0) {
        println!(
            "Pulled: {} created, {} updated, {} deleted",
            pulled.0, pulled.1, pulled.2
        );
    }

    if pushed != (0, 0, 0) {
        println!(
            "Pushed: {} created, {} updated, {} deleted",
            pushed.0, pushed.1, pushed.2
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn sync_connection(
    caldir: &Caldir,
    connection: &mut Connection,
    range: &DateRange,
    verbose: bool,
    force: bool,
    pulled: &mut Counts,
    pushed: &mut Counts,
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

    println!("{}", diff.render(verbose, caldir));

    match connection.apply_incoming_diff(&diff) {
        Ok(()) => add_counts(pulled, count_changes(diff.incoming())),
        Err(e) => println!("   {}", e.to_string().red()),
    }

    if !allow_mass_delete(&diff, force) {
        return;
    }

    match connection.apply_outgoing_diff(&diff).await {
        Ok(()) => add_counts(pushed, count_changes(diff.outgoing())),
        Err(e) => println!("   {}", e.to_string().red()),
    }
}

fn add_counts(acc: &mut Counts, delta: Counts) {
    acc.0 += delta.0;
    acc.1 += delta.1;
    acc.2 += delta.2;
}
