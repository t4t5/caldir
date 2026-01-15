//! The auth process will spawn a local HTTP server to receive the OAuth callback.

use anyhow::{Context, Result};
use google_calendar::Client;
use google_calendar::types::MinAccessRole;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::app_config::AppConfig;
use crate::session::{Session, SessionData};

pub const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

const DEFAULT_REDIRECT_PORT: u16 = 8085;

#[derive(Debug, Deserialize)]
struct AuthenticateParams {
    redirect_port: Option<u16>,
}

fn redirect_uri(port: u16) -> String {
    format!("http://localhost:{}/callback", port)
}

fn redirect_address(port: u16) -> String {
    format!("127.0.0.1:{}", port)
}

pub async fn handle(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: AuthenticateParams = serde_json::from_value(params)?;
    let port = params.redirect_port.unwrap_or(DEFAULT_REDIRECT_PORT);

    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();

    let app_config = AppConfig::load()?;

    let mut client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri(port),
        String::new(),
        String::new(),
    );

    let auth_url = client.user_consent_url(&scopes);

    eprintln!("\nOpen this URL in your browser to authenticate:\n");
    eprintln!("{}\n", auth_url);

    // Try to open the browser automatically
    if open::that(&auth_url).is_err() {
        eprintln!("(Could not open browser automatically, please copy the URL above)");
    }

    let (code, state) = wait_for_callback(port).await?;

    eprintln!("\nReceived authorization code, exchanging for tokens...");

    let tokens = client.get_access_token(&code, &state).await?;

    let session_data: SessionData = (&tokens).into();

    let client = Client::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        redirect_uri(port),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    );

    let calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await?
        .body;

    // user email (i.e. primary calendar)
    let account_email = calendars
        .iter()
        .find(|cal| cal.primary)
        .map(|cal| &cal.summary)
        .ok_or_else(|| anyhow::anyhow!("No primary calendar found"))?;

    // Save access_token + refresh_token in session file:
    let session = Session::new(&account_email, &session_data)?;
    session.save()?;

    eprintln!("Authentication successful!");

    Ok(serde_json::to_value(account_email)?)
}

async fn wait_for_callback(port: u16) -> Result<(String, String)> {
    let listener = TcpListener::bind(redirect_address(port))
        .await
        .context("Failed to bind OAuth callback listener")?;

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

    // Parse the request to get the code and state
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid HTTP request"))?;

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("No code in callback"))?;

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("No state in callback"))?;

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

    Ok((code, state))
}
