use anyhow::Result;

use crate::client::Client;
use crate::render;

pub async fn run(provider_name: &str) -> Result<()> {
    let spinner = render::create_spinner(format!("Authenticating with {provider_name}..."));

    let client = Client::connect().await?;
    let result = client.authenticate(provider_name).await;

    spinner.finish_and_clear();

    let response = result?;

    println!("Authenticated as: {}\n", response.account);

    if response.calendars_created.is_empty() && response.calendars_existing.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    let total = response.calendars_created.len() + response.calendars_existing.len();
    println!("Found {} calendar(s):\n", total);

    for name in &response.calendars_created {
        println!("  {name}/ (created)");
    }

    for name in &response.calendars_existing {
        println!("  {name}/ (already exists)");
    }

    println!("\nRun `caldir pull` to sync events.");

    Ok(())
}
