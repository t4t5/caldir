//! Handle the connect flow for Microsoft Outlook Calendar.
//!
//! Multi-step state machine:
//! 1. If no app_config and hosted=true → return HostedOAuth URL (caldir.org)
//! 2. If no app_config and hosted=false → return NeedsSetup (Azure AD app registration)
//! 3. If setup data submitted → save app_config, return OAuthRedirect URL
//! 4. If OAuth credentials submitted → exchange for tokens, GET /me, return Done

use anyhow::{Context, Result};
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, FieldType, HostedOAuthData,
    OAuthData, SetupData,
};
use url::Url;

use crate::app_config::AppConfig;
use crate::graph_client::GraphClient;
use crate::graph_types::GraphUser;
use crate::session::{AuthMode, Session, SessionData};

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

    let hosted = cmd
        .options
        .get("hosted")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Setup submit: client_id + client_secret without code/access_token
    let has_setup_fields = cmd.data.contains_key("client_id")
        && cmd.data.contains_key("client_secret")
        && !cmd.data.contains_key("code")
        && !cmd.data.contains_key("access_token");

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

    // Auth data submit step
    let has_auth_data =
        cmd.data.contains_key("code") || cmd.data.contains_key("access_token");

    if has_auth_data {
        let account_email = complete_auth(&cmd, &redirect_uri).await?;
        return Ok(ConnectResponse::Done {
            account_identifier: account_email,
        });
    }

    // Initial step: check if app config exists
    if !AppConfig::exists()? {
        if hosted {
            let port = Url::parse(&redirect_uri)?
                .port()
                .ok_or_else(|| anyhow::anyhow!("Could not extract port from redirect_uri"))?;

            return Ok(ConnectResponse::NeedsInput {
                step: ConnectStepKind::HostedOAuth,
                data: serde_json::to_value(HostedOAuthData {
                    url: format!("https://caldir.org/auth/outlook/start?port={}", port),
                })?,
            });
        } else {
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
    }

    // Self-hosted path: generate OAuth redirect URL
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

/// Complete authentication by exchanging credentials for tokens.
async fn complete_auth(cmd: &Connect, redirect_uri: &str) -> Result<String> {
    let (session_data, auth_mode, access_token) =
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

            let session_data = SessionData::from_hosted_tokens(
                access_token.to_string(),
                refresh_token.to_string(),
                expires_in,
            );

            (session_data, AuthMode::Hosted, access_token.to_string())
        } else {
            // Self-hosted flow: exchange authorization code for tokens
            let code = cmd
                .data
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'code' in credentials"))?;

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

            let session_data = SessionData::from_tokens(
                tokens.access_token.clone(),
                tokens.refresh_token,
                tokens.expires_in,
            );

            (session_data, AuthMode::Local, tokens.access_token)
        };

    // Get user email from /me
    let graph = GraphClient::new(&access_token);
    let me_response = graph.get("/me").await?;
    let user: GraphUser = me_response
        .json()
        .await
        .context("Failed to parse /me response")?;

    let account_email = user.email().to_string();

    let session = Session::new(&account_email, session_data, auth_mode);
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
