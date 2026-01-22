//! Complete OAuth authentication - exchanges code for tokens.

use anyhow::{Result, Context};
use caldir_core::remote::protocol::AuthSubmit;
use google_calendar::Client;
use google_calendar::types::MinAccessRole;

use crate::app_config::AppConfig;
use crate::session::{Session, SessionData};

pub async fn handle(cmd: AuthSubmit) -> Result<String> {
    let code = cmd
        .credentials
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'code' in credentials"))?;

    let state = cmd
        .credentials
        .get("state")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'state' in credentials"))?;

    let redirect_uri = cmd
        .credentials
        .get("redirect_uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'redirect_uri' in credentials"))?;

    let app_config = AppConfig::load()?;

    let mut client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri.to_string(),
        String::new(),
        String::new(),
    );

    let tokens = client
        .get_access_token(code, state)
        .await
        .context("Failed to exchange authorization code for tokens")?;

    let session_data: SessionData = (&tokens).into();

    // Create a new client with the tokens to fetch account info
    let client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri.to_string(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    );

    // Fetch calendars to get the user's email (primary calendar)
    let calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let account_email = calendars
        .iter()
        .find(|cal| cal.primary)
        .map(|cal| &cal.summary)
        .ok_or_else(|| anyhow::anyhow!("No primary calendar found"))?;

    // Save access_token + refresh_token in session file
    let session = Session::new(account_email, &session_data)?;
    session.save()?;

    Ok(account_email.clone())
}
