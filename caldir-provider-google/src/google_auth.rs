//! Google Calendar API implementation.

use crate::config;
use crate::types::{GoogleAccountTokens, GoogleCredentials};
use anyhow::{Context, Result};
use google_calendar::Client;

pub const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

const REDIRECT_PORT: u16 = 8085;

pub fn redirect_uri() -> String {
    format!("http://localhost:{}/callback", REDIRECT_PORT)
}

pub fn redirect_address() -> String {
    format!("127.0.0.1:{}", REDIRECT_PORT)
}

/// Get tokens for an account, refreshing if needed
pub async fn get_valid_tokens(email: &str) -> Result<GoogleAccountTokens> {
    let creds = config::load_credentials()?;
    let mut tokens = config::load_tokens(email)?;

    if config::tokens_need_refresh(&tokens) {
        eprintln!("Access token expired, refreshing..."); // TODO: Remove this
        tokens = refresh_token(&creds, &tokens).await?;
        config::save_tokens(email, &tokens)?;
    }

    Ok(tokens)
}

/// Create an authenticated Google Calendar client for the account.
pub async fn client_for_account(email: &str) -> Result<Client> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(email).await?;

    Ok(Client::new(
        creds.client_id,
        creds.client_secret,
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    ))
}

pub async fn refresh_token(
    creds: &GoogleCredentials,
    tokens: &GoogleAccountTokens,
) -> Result<GoogleAccountTokens> {
    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    );

    let access_token = client
        .refresh_access_token()
        .await
        .context("Failed to refresh token")?;

    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    // Google typically doesn't return a new refresh_token on refresh
    let refresh_token = if access_token.refresh_token.is_empty() {
        tokens.refresh_token.clone()
    } else {
        access_token.refresh_token
    };

    Ok(GoogleAccountTokens {
        access_token: access_token.access_token,
        refresh_token,
        expires_at,
    })
}
