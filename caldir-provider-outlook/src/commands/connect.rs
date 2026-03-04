//! Handle the connect flow for Microsoft Outlook Calendar.
//!
//! Three-step state machine:
//! 1. No app_config → NeedsSetup (Azure AD app registration)
//! 2. Setup submitted → save app_config, return OAuthRedirect URL
//! 3. OAuth code submitted → exchange for tokens, GET /me, return Done

use anyhow::{Context, Result};
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, FieldType, OAuthData, SetupData,
};
use url::Url;

use crate::app_config::AppConfig;
use crate::graph_client::GraphClient;
use crate::graph_types::GraphUser;
use crate::session::{Session, SessionData};

const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const SCOPES: &str = "Calendars.ReadWrite User.Read offline_access";

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    let redirect_uri = cmd
        .options
        .get("redirect_uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("redirect_uri is required for OAuth"))?
        .to_string();

    // Setup submit: client_id + client_secret without code
    let has_setup_fields = cmd.data.contains_key("client_id")
        && cmd.data.contains_key("client_secret")
        && !cmd.data.contains_key("code");

    if has_setup_fields {
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
        AppConfig {
            client_id,
            client_secret,
        }
        .save()?;
        // Fall through to generate OAuth URL
    }

    // Code exchange step
    if let Some(code) = cmd.data.get("code").and_then(|v| v.as_str()) {
        let account_email = exchange_code(code, &redirect_uri).await?;
        return Ok(ConnectResponse::Done {
            account_identifier: account_email,
        });
    }

    // Initial step: check if app config exists
    if !AppConfig::exists()? {
        return Ok(ConnectResponse::NeedsInput {
            step: ConnectStepKind::NeedsSetup,
            data: serde_json::to_value(SetupData {
                instructions: "To connect to Outlook Calendar, you need to register an Azure AD app:\n\n\
                    1. Go to https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps/ApplicationsListBlade\n\
                    2. Click \"New registration\"\n\
                    3. Name: \"caldir\" (or anything you like)\n\
                    4. Supported account types: \"Accounts in any organizational directory and personal Microsoft accounts\"\n\
                    5. Redirect URI: Select \"Web\" and enter http://localhost\n\
                    6. Click \"Register\"\n\
                    7. Copy the \"Application (client) ID\" → paste as Client ID below\n\
                    8. Go to \"Certificates & secrets\" → \"New client secret\" → copy the Value → paste as Client Secret below\n\
                    9. Go to \"API permissions\" → \"Add a permission\" → \"Microsoft Graph\" → \"Delegated permissions\"\n\
                    10. Add: Calendars.ReadWrite, User.Read, offline_access\n\
                    11. Click \"Grant admin consent\" (if available)".to_string(),
                fields: vec![
                    CredentialField {
                        id: "client_id".to_string(),
                        label: "Client ID (Application ID)".to_string(),
                        field_type: FieldType::Text,
                        required: true,
                        help: None,
                    },
                    CredentialField {
                        id: "client_secret".to_string(),
                        label: "Client Secret".to_string(),
                        field_type: FieldType::Password,
                        required: true,
                        help: None,
                    },
                ],
            })?,
        });
    }

    // Generate OAuth redirect URL
    let app_config = AppConfig::load()?;

    let state = format!("{:x}", rand_u64());

    let mut auth_url = Url::parse(AUTHORIZE_URL)?;
    auth_url.query_pairs_mut()
        .append_pair("client_id", &app_config.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("state", &state)
        .append_pair("response_mode", "query");

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::OAuthRedirect,
        data: serde_json::to_value(OAuthData {
            authorization_url: auth_url.to_string(),
            state,
            scopes: SCOPES.split(' ').map(String::from).collect(),
        })?,
    })
}

async fn exchange_code(code: &str, redirect_uri: &str) -> Result<String> {
    let app_config = AppConfig::load()?;

    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", app_config.client_id.as_str()),
            ("client_secret", app_config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("scope", SCOPES),
        ])
        .send()
        .await
        .context("Failed to exchange authorization code")?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed: {}", error);
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    }

    let tokens: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    // Get user email from /me
    let graph = GraphClient::new(&tokens.access_token);
    let me_response = graph.get("/me").await?;
    let user: GraphUser = me_response
        .json()
        .await
        .context("Failed to parse /me response")?;

    let account_email = user.email().to_string();

    let session_data = SessionData::from_tokens(
        tokens.access_token,
        tokens.refresh_token,
        tokens.expires_in,
    );
    let session = Session::new(&account_email, session_data);
    session.save()?;

    Ok(account_email)
}

/// Simple pseudo-random u64 for state parameter.
fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_nanos() as u64 ^ 0x517cc1b727220a95
}
