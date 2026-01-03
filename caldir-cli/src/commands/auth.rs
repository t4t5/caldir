use anyhow::Result;

use crate::provider::Provider;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::new(provider_name)?;

    println!("Authenticating with {}...", provider_name);

    // Provider handles the full OAuth flow and stores credentials/tokens
    let account = provider.authenticate().await?;

    println!("\nAuthenticated as: {}", account);
    println!("\nNow add a calendar to your config.toml:");
    println!();
    println!("[calendars.personal]");
    println!("provider = \"{}\"", provider_name);
    println!("{}_account = \"{}\"", provider_name, account);
    println!("{}_calendar_id = \"primary\"", provider_name);
    println!();
    println!("Then run `caldir-cli pull` to sync your calendar.");

    Ok(())
}
