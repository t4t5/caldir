//! Complete OAuth authentication - exchanges code for tokens or accepts tokens directly.

use anyhow::{Context, Result};
use caldir_core::remote::protocol::AuthSubmit;
use google_calendar::types::MinAccessRole;
use google_calendar::Client;

use crate::app_config::AppConfig;
use crate::session::{AuthMode, Session, SessionData};

pub async fn handle(cmd: AuthSubmit) -> Result<String> {
    // Determine flow: hosted (access_token present) vs self-hosted (code present)
    let (session_data, auth_mode, client) =
        if let Some(access_token) = cmd.credentials.get("access_token").and_then(|v| v.as_str()) {
            // Hosted flow: tokens already exchanged by caldir.org
            let refresh_token = cmd
                .credentials
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'refresh_token' in credentials"))?;

            let expires_in: i64 = cmd
                .credentials
                .get("expires_in")
                .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_i64()))
                .ok_or_else(|| anyhow::anyhow!("Missing 'expires_in' in credentials"))?;

            let session_data = SessionData::from_tokens(
                access_token.to_string(),
                refresh_token.to_string(),
                expires_in,
            );

            // For hosted mode, we don't need client_id/secret for API calls
            let client = Client::new(
                String::new(),
                String::new(),
                String::new(),
                access_token.to_string(),
                refresh_token.to_string(),
            );

            (session_data, AuthMode::Hosted, client)
        } else {
            // Self-hosted flow: exchange authorization code for tokens
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

            let client = Client::new(
                app_config.client_id.clone(),
                app_config.client_secret.clone(),
                redirect_uri.to_string(),
                tokens.access_token.clone(),
                tokens.refresh_token.clone(),
            );

            (session_data, AuthMode::Local, client)
        };

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
    let session = Session::new(account_email, &session_data, auth_mode)?;
    session.save()?;

    Ok(account_email.clone())
}
