use anyhow::Result;
use google_calendar::Client;
use google_calendar::types::MinAccessRole;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::config;

const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

const REDIRECT_PORT: u16 = 8085;

pub fn redirect_uri() -> String {
    format!("http://localhost:{}/callback", REDIRECT_PORT)
}

fn redirect_address() -> String {
    format!("127.0.0.1:{}", REDIRECT_PORT)
}

pub async fn handle_authenticate() -> Result<serde_json::Value> {
    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();

    let creds = config::load_credentials()?;

    let mut client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
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

    let (code, state) = wait_for_callback()?;

    eprintln!("\nReceived authorization code, exchanging for tokens...");

    let access_token = client.get_access_token(&code, &state).await?;

    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    let tokens = crate::types::GoogleAccountTokens {
        access_token: access_token.access_token,
        refresh_token: access_token.refresh_token,
        expires_at,
    };

    let creds = config::load_credentials()?;

    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    );

    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await?;

    let email = response
        .body
        .iter()
        .find(|cal| cal.primary)
        .map(|cal| cal.id.clone())
        .expect("No primary calendar found");

    // Save tokens for this account
    config::save_tokens(&email, &tokens)?;

    eprintln!("Authentication successful!");

    Ok(serde_json::to_value(email)?)
}

fn wait_for_callback() -> Result<(String, String)> {
    let listener = TcpListener::bind(redirect_address())?;

    let (mut stream, _) = listener.accept()?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request to get the code and state
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .expect("Invalid HTTP request");

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .expect("No code in callback");

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .expect("No state in callback");

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
