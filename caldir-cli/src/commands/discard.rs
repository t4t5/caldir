use anyhow::Result;
use caldir_core::{Caldir, CalendarDiff, Connection, DateRange, EventChange};
use dialoguer::Confirm;
use owo_colors::OwoColorize;

use crate::render::diff::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let all_connections = caldir.connections();

    let connections = match calendar {
        Some(cal) => all_connections
            .into_iter()
            .filter(|conn| conn.as_ref().ok().and_then(|c| c.local().slug()) == Some(cal.as_str()))
            .collect(),
        None => all_connections,
    };

    let range = DateRange::default();
    let mut pending: Vec<(Connection, CalendarDiff)> = Vec::new();
    let total = connections.len();

    for (i, connection) in connections.into_iter().enumerate() {
        match connection {
            Ok(connection) => {
                let header = connection.local().render(caldir);
                let spinner = tui::create_spinner(header.clone());
                let result = connection.diff(&range).await;
                spinner.finish_and_clear();

                println!("{}", header);

                match result {
                    Ok(diff) => {
                        println!("{}", diff.render_discard(verbose, caldir));
                        if !diff.outgoing().is_empty() {
                            pending.push((connection, diff));
                        }
                    }
                    Err(e) => println!("   {}", e.to_string().red()),
                }
            }
            Err(e) => println!("   {}", e.to_string().red()),
        }

        if i < total - 1 {
            println!();
        }
    }

    let total_changes: usize = pending.iter().map(|(_, d)| d.outgoing().len()).sum();

    if total_changes == 0 {
        return Ok(());
    }

    if !force {
        println!();
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Discard {} {}?",
                total_changes,
                if total_changes == 1 {
                    "change"
                } else {
                    "changes"
                }
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            return Ok(());
        }
    }

    for (connection, diff) in &pending {
        connection.local().discard_diff(diff)?;
    }

    let (created, updated, deleted) = pending.iter().fold((0, 0, 0), |(c, u, d), (_, diff)| {
        diff.outgoing()
            .iter()
            .fold((c, u, d), |(c, u, d), change| match change {
                EventChange::Create(_) => (c + 1, u, d),
                EventChange::Update { .. } => (c, u + 1, d),
                EventChange::Delete(_) => (c, u, d + 1),
            })
    });

    println!(
        "\nDiscarded {} {}: {} created, {} updated, {} deleted",
        total_changes,
        if total_changes == 1 {
            "change"
        } else {
            "changes"
        },
        created,
        updated,
        deleted
    );

    Ok(())
}
