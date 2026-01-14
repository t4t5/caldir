//! Google Calendar API implementation.

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

pub fn tokens_need_refresh(tokens: &GoogleAccountTokens) -> bool {
    tokens
        .expires_at
        .map(|exp| exp < chrono::Utc::now())
        .unwrap_or(false)
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
