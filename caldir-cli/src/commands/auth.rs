use anyhow::Result;
use indoc::printdoc;

use crate::provider::Provider;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::new(provider_name)?;

    println!("Authenticating with {provider_name}...");

    // Provider handles the full OAuth flow and stores credentials/tokens
    let account = provider.authenticate().await?;

    printdoc! {"

        Authenticated as: {account}

        Now add a calendar to your config.toml:

        [calendars.personal]
        provider = \"{provider_name}\"
        {provider_name}_account = \"{account}\"
        {provider_name}_calendar_id = \"primary\"

        Then run `caldir-cli pull` to sync your calendar.
    "}

    Ok(())
}
