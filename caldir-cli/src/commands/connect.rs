use anyhow::{Context, Result};
use caldir_core::rpc::{
    ConnectResponse, ConnectStepKind, CredentialsData, FieldType, HostedOAuthData, OAuthData,
    SetupData,
};
use caldir_core::{Caldir, Calendar, CalendarConfig, Connection, ProviderSlug};
use dialoguer::MultiSelect;
use std::collections::HashMap;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

fn build_options(hosted: bool, redirect_uri: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut options = serde_json::Map::new();
    options.insert("redirect_uri".into(), redirect_uri.into());
    options.insert("hosted".into(), hosted.into());
    options
}

pub async fn run(caldir: &mut Caldir, provider: Option<String>, hosted: bool) -> Result<()> {
    let provider_slug = provider.context(missing_provider_message(caldir))?;

    let provider_slug = ProviderSlug::from(provider_slug);

    run_parsed(caldir, provider_slug, hosted).await
}

fn missing_provider_message(caldir: &Caldir) -> String {
    let mut providers: Vec<String> = caldir
        .providers()
        .slugs()
        .into_iter()
        .map(ToString::to_string)
        .collect();
    providers.sort();

    let options = if providers.is_empty() {
        "  (none found in PATH)".to_string()
    } else {
        providers
            .into_iter()
            .map(|provider| format!("  {provider}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Missing provider argument.\n\nUsage:\n  caldir connect <provider>\n\nAvailable providers:\n{options}"
    )
}

async fn run_parsed(caldir: &mut Caldir, provider_slug: ProviderSlug, hosted: bool) -> Result<()> {
    let provider = caldir.provider(&provider_slug)?;

    // Bind to port 0 so the OS picks a free port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind OAuth callback listener")?;
    let port = listener.local_addr()?.port();

    // Build options:
    let redirect_uri = format!("http://localhost:{}/callback", port);
    let options = build_options(hosted, &redirect_uri);

    println!("Connecting to {}...\n", provider.slug());

    // Connect loop: keep calling `connect` until the provider says Done.
    let mut data = serde_json::Map::new();
    let (account_identifier, prefetched_calendars) = loop {
        let response = provider.connect(options.clone(), data.clone()).await?;

        match response {
            ConnectResponse::Done {
                account_identifier,
                calendars,
            } => {
                break (account_identifier, calendars);
            }
            ConnectResponse::NeedsInput {
                step,
                data: step_data,
            } => {
                data = run_connect_step(step, step_data, &listener, &redirect_uri).await?;
            }
        }
    };

    if let Some(id) = &account_identifier {
        println!("Authenticated as: {}\n", id);
    }

    // Single-calendar providers (webcal) return the calendar in `Done` and skip
    // list_calendars entirely. Multi-calendar account providers return an
    // account_identifier and we enumerate via list_calendars.
    let calendar_configs = if let Some(calendars) = prefetched_calendars {
        calendars
    } else {
        let id = account_identifier
            .clone()
            .context("Provider finished connecting without an account identifier or calendars")?;
        println!("Fetching calendars...");
        provider.provider_account(id).list_calendars().await?
    };

    if calendar_configs.is_empty() {
        println!("No calendars found.");
        return Ok(());
    }

    println!("Found {} calendar(s).\n", calendar_configs.len());

    // Skip calendars whose remote already matches a local one — keeps re-running
    // `connect` idempotent instead of spawning `personal-2/` next to `personal/`.
    let existing_connections: Vec<Connection> = caldir
        .connections()
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    let mut new_configs: Vec<CalendarConfig> = Vec::new();
    let mut skipped: Vec<(CalendarConfig, String)> = Vec::new();

    for cfg in calendar_configs {
        let existing_cal = cfg.remote_config().and_then(|remote_cfg| {
            existing_connections
                .iter()
                .find(|conn| conn.local().remote_config() == Some(remote_cfg))
        });
        match existing_cal {
            Some(conn) => {
                let slug = conn.local().slug().unwrap_or_default().to_string();
                skipped.push((cfg, slug));
            }
            None => new_configs.push(cfg),
        }
    }

    if !skipped.is_empty() {
        println!("Skipping {} already-connected calendar(s):", skipped.len());
        for (cfg, slug) in &skipped {
            let name = cfg.name().unwrap_or("Unnamed");
            println!("  - {name} ({slug}/)");
        }
        println!();
    }

    if new_configs.is_empty() {
        println!("All calendars for this account are already connected.");
        return Ok(());
    }
    let calendar_configs = new_configs;

    // Build selection items with calendar names
    let items: Vec<String> = calendar_configs
        .iter()
        .map(|c| {
            c.name()
                .map(String::from)
                .unwrap_or_else(|| "Unnamed".to_string())
        })
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
    let mut created_slugs: Vec<String> = Vec::new();

    for &idx in &selections {
        let config = &calendar_configs[idx];
        let desired_slug = Calendar::base_slug_for(config.name());
        let calendar = caldir.create_calendar(&desired_slug, Some(config.clone()))?;

        if let Some(slug) = calendar.slug() {
            println!("  {slug}/ (created)");
            created_slugs.push(slug.to_string());
        }
    }

    // Set the first writable calendar as default if none is configured yet
    let first_writable = selections.iter().enumerate().find_map(|(i, &idx)| {
        let config = &calendar_configs[idx];

        if config.read_only() != Some(true) {
            Some(created_slugs[i].clone())
        } else {
            None
        }
    });

    if let Some(slug) = first_writable
        && caldir.config().default_calendar_slug().is_none()
    {
        let mut config = caldir.config().clone();
        config.set_default_calendar_slug(Some(slug.to_string()));
        caldir.save_config(config)?;
    }

    println!("\nCalendars saved to {}\n", caldir.data_dir().display());

    if !created_slugs.is_empty() {
        println!("Pulling events...\n");
        super::pull::run(caldir, created_slugs, None, None, false).await?;
    }

    Ok(())
}

/// Run one iteration of the connect state machine: prompt the user or wait for an
/// OAuth callback, then return the data to send back to the provider on the next call.
async fn run_connect_step(
    step: ConnectStepKind,
    step_data: serde_json::Value,
    listener: &TcpListener,
    redirect_uri: &str,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    match step {
        ConnectStepKind::NeedsSetup => {
            let setup_data: SetupData = serde_json::from_value(step_data)
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

            println!("\nSetup complete. Continuing with connection...\n");
            Ok(fields)
        }
        ConnectStepKind::OAuthRedirect => {
            let oauth: OAuthData = serde_json::from_value(step_data)
                .context("Failed to parse OAuth data from provider")?;

            println!("Open this URL in your browser to authenticate:\n");
            println!("{}\n", oauth.authorization_url);

            if open::that(&oauth.authorization_url).is_err() {
                println!("(Could not open browser automatically, please copy the URL above)");
            }

            let params = wait_for_callback(listener).await?;

            let code = params
                .get("code")
                .ok_or_else(|| anyhow::anyhow!("No code in callback"))?;
            let state = params
                .get("state")
                .ok_or_else(|| anyhow::anyhow!("No state in callback"))?;

            if state != &oauth.state {
                anyhow::bail!("OAuth state mismatch - possible CSRF attack");
            }

            println!("\nReceived authorization code, exchanging for tokens...");

            let mut credentials = serde_json::Map::new();
            credentials.insert("code".into(), code.clone().into());
            credentials.insert("state".into(), state.clone().into());
            credentials.insert("redirect_uri".into(), redirect_uri.to_string().into());
            Ok(credentials)
        }
        ConnectStepKind::HostedOAuth => {
            let hosted_data: HostedOAuthData = serde_json::from_value(step_data)
                .context("Failed to parse hosted OAuth data from provider")?;

            println!("Open this URL in your browser to authenticate:\n");
            println!("{}\n", hosted_data.url);

            if open::that(&hosted_data.url).is_err() {
                println!("(Could not open browser automatically, please copy the URL above)");
            }

            let params = wait_for_callback(listener).await?;

            let access_token = params
                .get("access_token")
                .ok_or_else(|| anyhow::anyhow!("No access_token in callback"))?;
            let refresh_token = params
                .get("refresh_token")
                .ok_or_else(|| anyhow::anyhow!("No refresh_token in callback"))?;
            let expires_in = params
                .get("expires_in")
                .ok_or_else(|| anyhow::anyhow!("No expires_in in callback"))?;

            println!("\nReceived tokens, completing authentication...");

            let mut credentials = serde_json::Map::new();
            credentials.insert("access_token".into(), access_token.clone().into());
            credentials.insert("refresh_token".into(), refresh_token.clone().into());
            credentials.insert("expires_in".into(), expires_in.clone().into());
            Ok(credentials)
        }
        ConnectStepKind::Credentials => {
            let creds_data: CredentialsData = serde_json::from_value(step_data)
                .context("Failed to parse credentials data from provider")?;

            let mut credentials = serde_json::Map::new();
            for field in &creds_data.fields {
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
            Ok(credentials)
        }
    }
}

/// Wait for an HTTP callback on a pre-bound listener and return all query parameters.
async fn wait_for_callback(listener: &TcpListener) -> Result<HashMap<String, String>> {
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

    // Parse the request to extract query parameters
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid HTTP request"))?;

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();

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

    Ok(params)
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
