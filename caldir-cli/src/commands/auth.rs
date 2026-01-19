use anyhow::Result;
use caldir_core::calendar::{slugify, Calendar};
use caldir_core::remote::provider::Provider;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::from_name(provider_name);

    println!("Authenticating with {provider_name}...");

    // Provider handles the full OAuth flow and stores credentials/tokens
    let provider_account = provider.authenticate().await?;

    println!("Authenticated as: {}\n", { &provider_account.identifier });
    println!("Fetching calendars...");

    // List all calendars for this account
    let calendar_configs = provider_account.list_calendars().await?;

    if calendar_configs.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    println!("Found {} calendar(s):\n", calendar_configs.len());

    // Create local directories for each calendar
    for config in calendar_configs {
        // Derive directory name from calendar name
        let dir_name = config
            .name
            .as_ref()
            .map(|n| slugify(n))
            .unwrap_or_else(|| "calendar".to_string());

        let calendar = Calendar {
            dir_name: dir_name.clone(),
            config,
        };
        calendar.save_config()?;

        println!("  {dir_name}/ (created)");
    }

    println!("\nRun `caldir pull` to sync events.");

    Ok(())
}
