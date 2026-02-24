//! Initialize OAuth authentication - returns the authorization URL.

use anyhow::Result;
use caldir_core::remote::protocol::{
    AuthInit, AuthInitResponse, AuthType, CredentialField, FieldType, OAuthData, SetupData,
};
use google_calendar::Client;
use url::Url;

use crate::app_config::AppConfig;

pub const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

pub async fn handle(cmd: AuthInit) -> Result<AuthInitResponse> {
    let redirect_uri = cmd
        .redirect_uri
        .ok_or_else(|| anyhow::anyhow!("redirect_uri is required for OAuth"))?;

    if !AppConfig::exists()? {
        let setup_data = SetupData {
            instructions: "\
To connect to Google Calendar, you need to create OAuth credentials:\n\
\n\
  1. Go to https://console.cloud.google.com/apis/credentials\n\
  2. Create a new project (or select an existing one)\n\
  3. Click \"Create credentials\" → \"OAuth client ID\"\n\
  4. Choose \"Desktop app\" as the application type\n\
  5. Pick a name (e.g., \"Caldir\")\n\
  6. Copy the client ID and client secret below"
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

        return Ok(AuthInitResponse {
            auth_type: AuthType::NeedsSetup,
            data: serde_json::to_value(setup_data)?,
        });
    }

    let app_config = AppConfig::load()?;

    let client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri.clone(),
        String::new(),
        String::new(),
    );

    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();

    // The library generates the URL with its own state parameter
    let authorization_url = client.user_consent_url(&scopes);

    // Extract the state from the URL that the library generated
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

    Ok(AuthInitResponse {
        auth_type: AuthType::OAuthRedirect,
        data: serde_json::to_value(oauth_data)?,
    })
}
