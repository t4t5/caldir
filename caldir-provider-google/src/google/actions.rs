//! Google Calendar API implementation.

use crate::config;
use crate::google::api::{create_auth_client, create_client, refresh_token_internal};
use crate::types::AccountTokens;
use anyhow::{Context, Result};
use google_calendar::types::MinAccessRole;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

pub const REDIRECT_PORT: u16 = 8085;
const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

/// Start a local HTTP server to receive the OAuth callback
fn wait_for_callback() -> Result<(String, String)> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .with_context(|| format!("Failed to bind to port {}", REDIRECT_PORT))?;

    eprintln!("Waiting for OAuth callback on port {}...", REDIRECT_PORT);

    let (mut stream, _) = listener.accept().context("Failed to accept connection")?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request to get the code and state
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .context("Invalid request")?;

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .context("No code in callback")?;

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .context("No state in callback")?;

    // Send a response to the browser
    let response = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/html\r\n\
        Connection: close\r\n\
        \r\n\
        <html><body>\
        <h1>Authentication successful!</h1>\
        <p>You can close this window and return to the terminal.</p>\
        </body></html>";

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok((code, state))
}

/// Run the full OAuth authentication flow.
/// Returns the account email/identifier.
pub async fn authenticate() -> Result<String> {
    let creds = config::load_credentials()?;
    let mut client = create_auth_client(&creds);

    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();
    let auth_url = client.user_consent_url(&scopes);

    eprintln!("\nOpen this URL in your browser to authenticate:\n");
    eprintln!("{}\n", auth_url);

    // Try to open the browser automatically
    if open::that(&auth_url).is_err() {
        eprintln!("(Could not open browser automatically, please copy the URL above)");
    }

    let (code, state) = wait_for_callback()?;

    eprintln!("\nReceived authorization code, exchanging for tokens...");

    let access_token = client
        .get_access_token(&code, &state)
        .await
        .context("Failed to exchange code for tokens")?;

    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    let tokens = AccountTokens {
        access_token: access_token.access_token,
        refresh_token: access_token.refresh_token,
        expires_at,
    };

    // Discover the user's email
    let client = create_client(&creds, &tokens);
    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await?;

    let email = response
        .body
        .iter()
        .find(|cal| cal.primary)
        .map(|cal| cal.id.clone())
        .unwrap_or_else(|| "(unknown)".to_string());

    // Save tokens for this account
    config::save_tokens(&email, &tokens)?;

    eprintln!("Authentication successful!");

    Ok(email)
}

/// Get tokens for an account, refreshing if needed
pub async fn get_valid_tokens(account: &str) -> Result<AccountTokens> {
    let creds = config::load_credentials()?;
    let mut tokens = config::load_tokens(account)?;

    if config::tokens_need_refresh(&tokens) {
        eprintln!("Access token expired, refreshing..."); // TODO: Remove this
        tokens = refresh_token_internal(&creds, &tokens).await?;
        config::save_tokens(account, &tokens)?;
    }

    Ok(tokens)
}
