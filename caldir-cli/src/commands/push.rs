use anyhow::Result;
use owo_colors::OwoColorize;

use caldir_lib::diff::DiffKind;

use crate::client::Client;
use crate::render::{self, Render};

pub async fn run() -> Result<()> {
    let spinner = render::create_spinner("Pushing to remote...".to_string());

    let client = Client::connect().await?;
    let results = client.push().await;

    spinner.finish_and_clear();

    let results = results?;

    let mut total_created = 0;
    let mut total_updated = 0;
    let mut total_deleted = 0;

    for (i, result) in results.iter().enumerate() {
        println!("{}", format!("{}:", result.calendar).bold());

        if let Some(ref error) = result.error {
            println!("   {}", error.red());
        } else {
            println!("{}", result.events.render());

            for event in &result.events {
                match event.kind {
                    DiffKind::Create => total_created += 1,
                    DiffKind::Update => total_updated += 1,
                    DiffKind::Delete => total_deleted += 1,
                }
            }
        }

        if i < results.len() - 1 {
            println!();
        }
    }

    if total_created > 0 || total_updated > 0 || total_deleted > 0 {
        println!(
            "\nPushed: {} created, {} updated, {} deleted",
            total_created, total_updated, total_deleted
        );
    }

    Ok(())
}
