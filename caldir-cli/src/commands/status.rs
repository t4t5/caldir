use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::render::{self, Render};

pub async fn run() -> Result<()> {
    let spinner = render::create_spinner("Checking status...".to_string());

    let client = Client::connect().await?;
    let results = client.status().await;

    spinner.finish_and_clear();

    let results = results?;

    for (i, result) in results.iter().enumerate() {
        println!("{}", format!("{}:", result.calendar).bold());

        if let Some(ref error) = result.error {
            println!("   {}", error.red());
        } else if result.to_push.is_empty() && result.to_pull.is_empty() {
            println!("   {}", "No changes".dimmed());
        } else {
            if !result.to_push.is_empty() {
                println!("   {}", "Local changes (to push):".dimmed());
                for diff in &result.to_push {
                    println!("   {}", diff.render());
                }
            }

            if !result.to_pull.is_empty() {
                if !result.to_push.is_empty() {
                    println!();
                }
                println!("   {}", "Remote changes (to pull):".dimmed());
                for diff in &result.to_pull {
                    println!("   {}", diff.render());
                }
            }
        }

        if i < results.len() - 1 {
            println!();
        }
    }

    Ok(())
}
