use anyhow::Result;
use caldir_core::provider::Provider;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::from_name(provider_name);

    println!("Authenticating with {provider_name}...");

    // Provider handles the full OAuth flow and stores credentials/tokens
    let provider_account = provider.authenticate().await?;

    println!("Authenticated as: {}\n", { &provider_account.identifier });
    println!("Fetching calendars...");

    // List all calendars for this account
    let calendars = provider_account.list_calendars().await?;

    if calendars.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    println!("Found {} calendar(s):\n", calendars.len());

    // Create local directories for each calendar
    for calendar in calendars {
        calendar.save_config()?;

        println!("  {}/ (created)", { calendar.name });
    }

    println!("\nRun `caldir pull` to sync events.");

    Ok(())
}
