use anyhow::{Context, Result};
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::remote::protocol::{AuthType, CredentialsData, FieldType, OAuthData, SetupData};
use caldir_core::remote::provider::Provider;
use dialoguer::MultiSelect;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const DEFAULT_REDIRECT_PORT: u16 = 8085;

pub async fn run(provider_name: &str) -> Result<()> {
    let provider = Provider::from_name(provider_name);
    let port = DEFAULT_REDIRECT_PORT;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    println!("Authenticating with {provider_name}...\n");

    // Phase 1: Get auth requirements from provider
    let mut auth_info = provider.auth_init(Some(redirect_uri.clone())).await?;

    // Handle one-time setup if provider needs it
    if matches!(auth_info.auth_type, AuthType::NeedsSetup) {
        let setup_data: SetupData = serde_json::from_value(auth_info.data)
            .context("Failed to parse setup data from provider")?;

        println!("{}\n", setup_data.instructions);

        let mut fields = serde_json::Map::new();

        for field in &setup_data.fields {
            if let Some(ref help) = field.help {
                println!("{}", help);
            }

            let value = match field.field_type {
                FieldType::Password => prompt_password(&field.label)?,
                _ => prompt_text(&field.label)?,
            };

            fields.insert(field.id.clone(), value.into());
        }

        provider.setup_submit(fields).await?;

        println!("\nSetup complete. Continuing with authentication...\n");

        // Retry auth_init now that setup is done
        auth_info = provider.auth_init(Some(redirect_uri.clone())).await?;
    }

    let provider_account = match auth_info.auth_type {
        AuthType::OAuthRedirect => {
            let oauth: OAuthData = serde_json::from_value(auth_info.data)
                .context("Failed to parse OAuth data from provider")?;

            println!("Open this URL in your browser to authenticate:\n");
            println!("{}\n", oauth.authorization_url);

            // Try to open the browser automatically
            if open::that(&oauth.authorization_url).is_err() {
                println!("(Could not open browser automatically, please copy the URL above)");
            }

            // Wait for OAuth callback
            let (code, state) = wait_for_callback(port).await?;

            // Validate state
            if state != oauth.state {
                anyhow::bail!("OAuth state mismatch - possible CSRF attack");
            }

            println!("\nReceived authorization code, exchanging for tokens...");

            // Phase 2: Submit credentials to complete auth
            let mut credentials = serde_json::Map::new();
            credentials.insert("code".into(), code.into());
            credentials.insert("state".into(), state.into());
            credentials.insert("redirect_uri".into(), redirect_uri.into());

            provider.auth_submit(credentials).await?
        }
        AuthType::Credentials => {
            let creds_data: CredentialsData = serde_json::from_value(auth_info.data)
                .context("Failed to parse credentials data from provider")?;

            let mut credentials = serde_json::Map::new();

            for field in &creds_data.fields {
                // Show help text if available
                if let Some(ref help) = field.help {
                    println!("{}", help);
                }

                let value = match field.field_type {
                    FieldType::Password => prompt_password(&field.label)?,
                    _ => prompt_text(&field.label)?,
                };

                credentials.insert(field.id.clone(), value.into());
            }

            println!("\nValidating credentials...");

            provider.auth_submit(credentials).await?
        }
        AuthType::NeedsSetup => {
            anyhow::bail!("Provider still requires setup after setup_submit — this is a provider bug");
        }
    };

    println!("Authenticated as: {}\n", provider_account.identifier);
    println!("Fetching calendars...");

    // List all calendars for this account
    let calendar_configs = provider_account.list_calendars().await?;

    if calendar_configs.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    println!("Found {} calendar(s).\n", calendar_configs.len());

    // Build selection items with calendar names
    let items: Vec<String> = calendar_configs
        .iter()
        .map(|c| c.name.clone().unwrap_or_else(|| "Unnamed".to_string()))
        .collect();

    // Show multi-select (all selected by default)
    let defaults: Vec<bool> = vec![true; items.len()];
    let selections = MultiSelect::new()
        .with_prompt("Select calendars to import (space to toggle, enter to confirm)")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    if selections.is_empty() {
        println!("No calendars selected.");
        return Ok(());
    }

    println!();

    // Create only selected calendars
    for idx in selections {
        let config = &calendar_configs[idx];
        let slug = Calendar::unique_slug_for(config.name.as_deref())?;

        let calendar = Calendar {
            slug: slug.clone(),
            config: config.clone(),
        };

        calendar.save_config()?;

        println!("  {slug}/ (created)");
    }

    let caldir = Caldir::load()?;
    println!(
        "\nCalendars saved to {}",
        caldir.data_path().display()
    );
    println!("Run `caldir pull` to sync events.");

    Ok(())
}

async fn wait_for_callback(port: u16) -> Result<(String, String)> {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&address)
        .await
        .context("Failed to bind OAuth callback listener")?;

    let (stream, _) = listener
        .accept()
        .await
        .context("Failed to accept OAuth callback")?;

    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .context("Failed to read OAuth callback request line")?;

    // Parse the request to get the code and state
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid HTTP request"))?;

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("No code in callback"))?;

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("No state in callback"))?;

    // Send a response to the browser
    let response = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/html\r\n\
        Connection: close\r\n\
        \r\n\
        <html><body>\
        <h1>Authentication successful!</h1>\
        <p>You can close this window and return to the terminal.</p>\
        </body></html>";

    let mut stream = reader.into_inner();
    stream
        .write_all(response.as_bytes())
        .await
        .context("Failed to write OAuth callback response")?;
    stream.flush().await?;

    Ok((code, state))
}

/// Prompt the user for text input.
fn prompt_text(label: &str) -> Result<String> {
    print!("{}: ", label);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

/// Prompt the user for password input (hidden).
fn prompt_password(label: &str) -> Result<String> {
    let prompt = format!("{}: ", label);
    rpassword::prompt_password(&prompt).context("Failed to read password")
}
