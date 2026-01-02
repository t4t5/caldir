//! caldir-provider-google - Google Calendar provider for caldir-cli
//!
//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.
//!
//! The provider manages its own credentials and tokens:
//!   ~/.config/caldir/providers/google/credentials.json
//!   ~/.config/caldir/providers/google/tokens/{account}.json

mod config;
mod google;
mod types;

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

/// Request from caldir-cli
#[derive(Debug, Deserialize)]
struct Request {
    command: String,
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
        "authenticate" => handle_authenticate().await,
        "fetch_calendars" => handle_fetch_calendars(&request.params).await,
        "fetch_events" => handle_fetch_events(&request.params).await,
        "create_event" => handle_create_event(&request.params).await,
        "update_event" => handle_update_event(&request.params).await,
        "delete_event" => handle_delete_event(&request.params).await,
        _ => error(&format!("Unknown command: {}", request.command)),
    }
}

async fn handle_authenticate() -> String {
    match google::authenticate().await {
        Ok(account) => success(account),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct FetchCalendarsParams {
    google_account: String,
}

async fn handle_fetch_calendars(params: &serde_json::Value) -> String {
    let params: FetchCalendarsParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    match google::fetch_calendars(&params.google_account).await {
        Ok(calendars) => success(calendars),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct FetchEventsParams {
    google_account: String,
    google_calendar_id: Option<String>,
    #[serde(default)]
    time_min: Option<String>,
    #[serde(default)]
    time_max: Option<String>,
}

async fn handle_fetch_events(params: &serde_json::Value) -> String {
    let params: FetchEventsParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    // Default to primary calendar if not specified
    let calendar_id = params.google_calendar_id.as_deref().unwrap_or("primary");

    match google::fetch_events(
        &params.google_account,
        calendar_id,
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
    google_account: String,
    google_calendar_id: Option<String>,
    event: types::Event,
}

async fn handle_create_event(params: &serde_json::Value) -> String {
    let params: CreateEventParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params.google_calendar_id.as_deref().unwrap_or("primary");

    match google::create_event(&params.google_account, calendar_id, &params.event).await {
        Ok(event) => success(event),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: types::Event,
}

async fn handle_update_event(params: &serde_json::Value) -> String {
    let params: UpdateEventParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params.google_calendar_id.as_deref().unwrap_or("primary");

    match google::update_event(&params.google_account, calendar_id, &params.event).await {
        Ok(()) => success(()),
        Err(e) => error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct DeleteEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event_id: String,
}

async fn handle_delete_event(params: &serde_json::Value) -> String {
    let params: DeleteEventParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params.google_calendar_id.as_deref().unwrap_or("primary");

    match google::delete_event(&params.google_account, calendar_id, &params.event_id).await {
        Ok(()) => success(()),
        Err(e) => error(&format!("{:#}", e)),
    }
}
