//! caldir-provider-google - Google Calendar provider for caldir-cli
//!
//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.

mod google;
mod types;

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use types::{AccountTokens, GoogleConfig};

/// Request from caldir-cli
#[derive(Debug, Deserialize)]
struct Request {
    command: String,
    config: GoogleConfig,
    #[serde(default)]
    tokens: Option<AccountTokens>,
    #[serde(default)]
    params: serde_json::Value,
}

/// Success response
#[derive(Debug, Serialize)]
struct SuccessResponse<T: Serialize> {
    status: &'static str,
    data: T,
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    status: &'static str,
    error: String,
}

/// Response with updated tokens
#[derive(Debug, Serialize)]
struct TokensUpdatedResponse<T: Serialize> {
    status: &'static str,
    tokens: AccountTokens,
    data: T,
}

fn success<T: Serialize>(data: T) -> String {
    serde_json::to_string(&SuccessResponse {
        status: "success",
        data,
    })
    .unwrap()
}

fn error(msg: &str) -> String {
    serde_json::to_string(&ErrorResponse {
        status: "error",
        error: msg.to_string(),
    })
    .unwrap()
}

#[allow(dead_code)]
fn tokens_updated<T: Serialize>(tokens: AccountTokens, data: T) -> String {
    serde_json::to_string(&TokensUpdatedResponse {
        status: "tokens_updated",
        tokens,
        data,
    })
    .unwrap()
}

#[tokio::main]
async fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to read stdin: {}", e);
                break;
            }
        };

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = error(&format!("Failed to parse request: {}", e));
                writeln!(stdout, "{}", response).unwrap();
                stdout.flush().unwrap();
                continue;
            }
        };

        let response = handle_request(request).await;

        writeln!(stdout, "{}", response).unwrap();
        stdout.flush().unwrap();
    }
}

async fn handle_request(request: Request) -> String {
    match request.command.as_str() {
        "authenticate" => handle_authenticate(&request.config).await,
        "refresh_token" => handle_refresh_token(&request).await,
        "fetch_user_email" => handle_fetch_user_email(&request).await,
        "fetch_calendars" => handle_fetch_calendars(&request).await,
        "fetch_events" => handle_fetch_events(&request).await,
        "create_event" => handle_create_event(&request).await,
        "update_event" => handle_update_event(&request).await,
        "delete_event" => handle_delete_event(&request).await,
        _ => error(&format!("Unknown command: {}", request.command)),
    }
}

async fn handle_authenticate(config: &GoogleConfig) -> String {
    match google::authenticate(config).await {
        Ok(tokens) => success(tokens),
        Err(e) => error(&format!("{:#}", e)),
    }
}

async fn handle_refresh_token(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("refresh_token requires tokens"),
    };

    match google::refresh_token(&request.config, tokens).await {
        Ok(new_tokens) => success(new_tokens),
        Err(e) => error(&format!("{:#}", e)),
    }
}

async fn handle_fetch_user_email(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("fetch_user_email requires tokens"),
    };

    match google::fetch_user_email(&request.config, tokens).await {
        Ok(email) => success(email),
        Err(e) => error(&format!("{:#}", e)),
    }
}

async fn handle_fetch_calendars(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("fetch_calendars requires tokens"),
    };

    match google::fetch_calendars(&request.config, tokens).await {
        Ok(calendars) => success(calendars),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct FetchEventsParams {
    calendar_id: String,
    #[serde(default)]
    time_min: Option<String>,
    #[serde(default)]
    time_max: Option<String>,
}

async fn handle_fetch_events(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("fetch_events requires tokens"),
    };

    let params: FetchEventsParams = match serde_json::from_value(request.params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    match google::fetch_events(
        &request.config,
        tokens,
        &params.calendar_id,
        params.time_min.as_deref(),
        params.time_max.as_deref(),
    )
    .await
    {
        Ok(events) => success(events),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct CreateEventParams {
    calendar_id: String,
    event: types::Event,
}

async fn handle_create_event(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("create_event requires tokens"),
    };

    let params: CreateEventParams = match serde_json::from_value(request.params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    match google::create_event(&request.config, tokens, &params.calendar_id, &params.event).await {
        Ok(event) => success(event),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateEventParams {
    calendar_id: String,
    event: types::Event,
}

async fn handle_update_event(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("update_event requires tokens"),
    };

    let params: UpdateEventParams = match serde_json::from_value(request.params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    match google::update_event(&request.config, tokens, &params.calendar_id, &params.event).await {
        Ok(()) => success(()),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct DeleteEventParams {
    calendar_id: String,
    event_id: String,
}

async fn handle_delete_event(request: &Request) -> String {
    let tokens = match &request.tokens {
        Some(t) => t,
        None => return error("delete_event requires tokens"),
    };

    let params: DeleteEventParams = match serde_json::from_value(request.params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    match google::delete_event(&request.config, tokens, &params.calendar_id, &params.event_id)
        .await
    {
        Ok(()) => success(()),
        Err(e) => error(&format!("{:#}", e)),
    }
}
