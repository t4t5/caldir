use std::collections::HashMap;

use anyhow::Result;

use crate::caldir::Caldir;
use crate::local::{LocalConfig, RemoteConfig};
use crate::provider::Provider;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::from_name(provider_name);
    let caldir = Caldir::load()?;

    println!("Authenticating with {provider_name}...");

    // Provider handles the full OAuth flow and stores credentials/tokens
    let account = provider.authenticate().await?;

    println!("Authenticated as: {account}\n");
    println!("Fetching calendars...");

    // List all calendars for this account
    let calendars = provider.list_calendars(&account).await?;

    if calendars.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    println!("Found {} calendar(s):\n", calendars.len());

    // Create local directories for each calendar
    for cal in calendars {
        let dir_name = slugify(&cal.name);
        let cal_path = caldir.data_path().join(&dir_name);

        // Skip if already exists
        if cal_path.join(".caldir/config.toml").exists() {
            println!("  {dir_name}/ (already exists)");
            continue;
        }

        // Create directory structure
        std::fs::create_dir_all(cal_path.join(".caldir/state"))?;

        // Build provider-specific params
        let mut params = HashMap::new();
        params.insert(
            format!("{}_account", provider.name()),
            toml::Value::String(account.clone()),
        );
        // Only add calendar_id if not primary (primary is the default)
        if !cal.primary {
            params.insert(
                format!("{}_calendar_id", provider.name()),
                toml::Value::String(cal.id),
            );
        }

        // Save config
        let config = LocalConfig {
            remote: Some(RemoteConfig {
                provider: provider_name.to_string(),
                params,
            }),
        };
        config.save(&cal_path)?;

        println!("  {dir_name}/ (created)");
    }

    println!("\nRun `caldir pull` to sync events.");

    Ok(())
}

/// Convert a calendar name to a directory-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
