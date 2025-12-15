use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

use crate::config::{GcalConfig, GcalTokens};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REDIRECT_PORT: u16 = 8085;
const REDIRECT_URI: &str = "http://localhost:8085/callback";

const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/userinfo.email",
];

/// Generate a random string for OAuth state/verifier
fn generate_random_string(len: usize) -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..len).map(|_| rng.gen()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

/// Generate PKCE code verifier and challenge
fn generate_pkce() -> (String, String) {
    let verifier = generate_random_string(32);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

/// Build the OAuth authorization URL
fn build_auth_url(config: &GcalConfig, state: &str, code_challenge: &str) -> String {
    let mut url = Url::parse(AUTH_URL).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("response_type", "code")
        .append_pair("scope", &SCOPES.join(" "))
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    url.to_string()
}

/// Start a local HTTP server to receive the OAuth callback
fn wait_for_callback(expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .with_context(|| format!("Failed to bind to port {}", REDIRECT_PORT))?;

    println!("Waiting for OAuth callback on port {}...", REDIRECT_PORT);

    let (mut stream, _) = listener.accept().context("Failed to accept connection")?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request to get the code
    // Request line looks like: GET /callback?code=xxx&state=yyy HTTP/1.1
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .context("Invalid request")?;

    let url = Url::parse(&format!("http://localhost{}", url_part))?;

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

    if state != expected_state {
        anyhow::bail!("State mismatch in OAuth callback");
    }

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

    Ok(code)
}

/// Exchange authorization code for tokens
async fn exchange_code(
    config: &GcalConfig,
    code: &str,
    code_verifier: &str,
) -> Result<GcalTokens> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("code", code),
        ("code_verifier", code_verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", REDIRECT_URI),
    ];

    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("Failed to exchange code for tokens")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed: {}", error_text);
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now() + chrono::Duration::seconds(secs as i64)
    });

    Ok(GcalTokens {
        access_token: token_response.access_token,
        refresh_token: token_response
            .refresh_token
            .context("No refresh token in response")?,
        expires_at,
    })
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

/// Refresh an expired access token
pub async fn refresh_token(config: &GcalConfig, tokens: &GcalTokens) -> Result<GcalTokens> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("refresh_token", tokens.refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];

    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("Failed to refresh token")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Token refresh failed: {}", error_text);
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now() + chrono::Duration::seconds(secs as i64)
    });

    Ok(GcalTokens {
        access_token: token_response.access_token,
        // Keep the old refresh token if a new one wasn't provided
        refresh_token: token_response
            .refresh_token
            .unwrap_or_else(|| tokens.refresh_token.clone()),
        expires_at,
    })
}

/// Run the full OAuth authentication flow
pub async fn authenticate(config: &GcalConfig) -> Result<GcalTokens> {
    let state = generate_random_string(16);
    let (code_verifier, code_challenge) = generate_pkce();

    let auth_url = build_auth_url(config, &state, &code_challenge);

    println!("\nOpen this URL in your browser to authenticate:\n");
    println!("{}\n", auth_url);

    // Try to open the browser automatically
    if let Err(_) = open::that(&auth_url) {
        println!("(Could not open browser automatically, please copy the URL above)");
    }

    // Wait for the callback
    let code = wait_for_callback(&state)?;

    println!("\nReceived authorization code, exchanging for tokens...");

    // Exchange code for tokens
    let tokens = exchange_code(config, &code, &code_verifier).await?;

    println!("Authentication successful!");

    Ok(tokens)
}

/// Fetch the user's email to verify authentication
pub async fn fetch_user_email(tokens: &GcalTokens) -> Result<String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .context("Failed to fetch user info")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch user info: {}", error_text);
    }

    #[derive(serde::Deserialize)]
    struct UserInfo {
        email: Option<String>,
    }

    let user_info: UserInfo = response.json().await?;
    user_info.email.context("No email in user info response")
}

/// Fetch the list of calendars for the authenticated user
pub async fn fetch_calendars(tokens: &GcalTokens) -> Result<Vec<Calendar>> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .context("Failed to fetch calendars")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch calendars: {}", error_text);
    }

    #[derive(serde::Deserialize)]
    struct CalendarListResponse {
        items: Option<Vec<CalendarItem>>,
    }

    #[derive(serde::Deserialize)]
    struct CalendarItem {
        id: String,
        summary: String,
        primary: Option<bool>,
    }

    let list: CalendarListResponse = response.json().await?;

    Ok(list
        .items
        .unwrap_or_default()
        .into_iter()
        .map(|c| Calendar {
            id: c.id,
            name: c.summary,
            primary: c.primary.unwrap_or(false),
        })
        .collect())
}

#[derive(Debug)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}
