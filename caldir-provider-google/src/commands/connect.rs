//! Handle the connect flow for Google Calendar.
//!
//! This single handler drives a multi-step state machine:
//! 1. If no app_config.toml and hosted=true → return HostedOAuth URL
//! 2. If no app_config.toml and hosted=false → return NeedsSetup (credential fields)
//! 3. If setup data is submitted → save app_config.toml, return OAuthRedirect URL
//! 4. If OAuth credentials are submitted → exchange for tokens, return Done

use anyhow::{Context, Result};
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, FieldType, HostedOAuthData,
    OAuthData, SetupData,
};
use google_calendar::types::MinAccessRole;
use google_calendar::Client;
use url::Url;

use crate::app_config::AppConfig;
use crate::session::{AuthMode, Session, SessionData};

pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar.calendarlist.readonly",
    "https://www.googleapis.com/auth/calendar.events.owned",
];

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    let redirect_uri = cmd
        .options
        .get("redirect_uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("redirect_uri is required for OAuth"))?
        .to_string();

    let hosted = cmd
        .options
        .get("hosted")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // If data contains credentials/tokens, this is a submit step.
    // Check for setup fields first (client_id + client_secret without code/access_token).
    let has_setup_fields = cmd.data.contains_key("client_id")
        && cmd.data.contains_key("client_secret")
        && !cmd.data.contains_key("code")
        && !cmd.data.contains_key("access_token");

    if has_setup_fields {
        // Setup submit: save OAuth credentials
        let client_id = cmd
            .data
            .get("client_id")
            .and_then(|v| v.as_str())
            .context("Missing client_id")?
            .to_string();
        let client_secret = cmd
            .data
            .get("client_secret")
            .and_then(|v| v.as_str())
            .context("Missing client_secret")?
            .to_string();

        let app_config = AppConfig {
            client_id,
            client_secret,
        };
        app_config.save()?;

        // Now fall through to generate the OAuth URL
    }

    let has_auth_data =
        cmd.data.contains_key("code") || cmd.data.contains_key("access_token");

    if has_auth_data {
        // Auth submit: exchange credentials for tokens
        let account_email = complete_auth(&cmd, &redirect_uri).await?;
        return Ok(ConnectResponse::Done {
            account_identifier: account_email,
        });
    }

    // Init step: determine what auth method to use
    if !AppConfig::exists()? {
        if hosted {
            let port = Url::parse(&redirect_uri)?
                .port()
                .ok_or_else(|| anyhow::anyhow!("Could not extract port from redirect_uri"))?;

            let hosted_data = HostedOAuthData {
                url: format!("https://caldir.org/auth/google/start?port={}", port),
            };

            return Ok(ConnectResponse::NeedsInput {
                step: ConnectStepKind::HostedOAuth,
                data: serde_json::to_value(hosted_data)?,
            });
        } else {
            let setup_data = SetupData {
                instructions: "\
To connect to Google Calendar, you need to create OAuth credentials:\n\
\n\
  1. Go to https://console.cloud.google.com/apis/credentials\n\
  2. Create a new project (or select an existing one)\n\
  3. Enable the Google Calendar API for your project: https://console.developers.google.com/apis/api/calendar-json.googleapis.com\n\
  4. Add your own account as a test user in the \"Audience\" tab\n\
  5. Click \"Create credentials\" → \"OAuth client ID\"\n\
  6. Choose \"Desktop app\" as the application type\n\
  7. Pick a name (e.g., \"Caldir\")\n\
  8. Copy the client ID and client secret below"
                    .to_string(),
                fields: vec![
                    CredentialField {
                        id: "client_id".to_string(),
                        label: "Client ID".to_string(),
                        field_type: FieldType::Text,
                        required: true,
                        help: None,
                    },
                    CredentialField {
                        id: "client_secret".to_string(),
                        label: "Client secret".to_string(),
                        field_type: FieldType::Text,
                        required: true,
                        help: None,
                    },
                ],
            };

            return Ok(ConnectResponse::NeedsInput {
                step: ConnectStepKind::NeedsSetup,
                data: serde_json::to_value(setup_data)?,
            });
        }
    }

    // Self-hosted path: user has their own OAuth credentials
    let app_config = AppConfig::load()?;

    let client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri.clone(),
        String::new(),
        String::new(),
    );

    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();

    let authorization_url = client.user_consent_url(&scopes);

    let parsed_url = Url::parse(&authorization_url)?;
    let state = parsed_url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("No state parameter in authorization URL"))?;

    let oauth_data = OAuthData {
        authorization_url,
        state,
        scopes,
    };

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::OAuthRedirect,
        data: serde_json::to_value(oauth_data)?,
    })
}

/// Complete authentication by exchanging credentials for tokens.
async fn complete_auth(cmd: &Connect, redirect_uri: &str) -> Result<String> {
    let (session_data, auth_mode, client) =
        if let Some(access_token) = cmd.data.get("access_token").and_then(|v| v.as_str()) {
            // Hosted flow: tokens already exchanged by caldir.org
            let refresh_token = cmd
                .data
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'refresh_token' in credentials"))?;

            let expires_in: i64 = cmd
                .data
                .get("expires_in")
                .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_i64()))
                .ok_or_else(|| anyhow::anyhow!("Missing 'expires_in' in credentials"))?;

            let session_data = SessionData::from_tokens(
                access_token.to_string(),
                refresh_token.to_string(),
                expires_in,
            );

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
                .data
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'code' in credentials"))?;

            let state = cmd
                .data
                .get("state")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'state' in credentials"))?;

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

    let session = Session::new(account_email, &session_data, auth_mode)?;
    session.save()?;

    Ok(account_email.clone())
}
