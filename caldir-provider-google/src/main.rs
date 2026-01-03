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

use caldir_core::protocol::{Command, Request, Response};
use serde::Deserialize;
use std::io::{self, BufRead, Write};

/// Google's alias for the user's main calendar
const DEFAULT_CALENDAR_ID: &str = "primary";

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
                let response = Response::error(&format!("Failed to parse request: {}", e));
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
    match request.command {
        Command::Authenticate => handle_authenticate().await,
        Command::ListCalendars => handle_list_calendars(&request.params).await,
        Command::ListEvents => handle_list_events(&request.params).await,
        Command::CreateEvent => handle_create_event(&request.params).await,
        Command::UpdateEvent => handle_update_event(&request.params).await,
        Command::DeleteEvent => handle_delete_event(&request.params).await,
    }
}

async fn handle_authenticate() -> String {
    match google::authenticate().await {
        Ok(account) => Response::success(account),
        Err(e) => Response::error(&format!("{:#}", e)),
    }
}

#[derive(Debug, Deserialize)]
struct FetchCalendarsParams {
    google_account: String,
}

async fn handle_list_calendars(params: &serde_json::Value) -> String {
    let params: FetchCalendarsParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return Response::error(&format!("Invalid params: {}", e)),
    };

    match google::fetch_calendars(&params.google_account).await {
        Ok(calendars) => Response::success(calendars),
        Err(e) => Response::error(&format!("{:#}", e)),
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

async fn handle_list_events(params: &serde_json::Value) -> String {
    let params: FetchEventsParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return Response::error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    match google::fetch_events(
        &params.google_account,
        calendar_id,
        params.time_min.as_deref(),
        params.time_max.as_deref(),
    )
    .await
    {
        Ok(events) => Response::success(events),
        Err(e) => Response::error(&format!("{:#}", e)),
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
        Err(e) => return Response::error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    match google::create_event(&params.google_account, calendar_id, &params.event).await {
        Ok(event) => Response::success(event),
        Err(e) => Response::error(&format!("{:#}", e)),
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
        Err(e) => return Response::error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    match google::update_event(&params.google_account, calendar_id, &params.event).await {
        Ok(()) => Response::success(()),
        Err(e) => Response::error(&format!("{:#}", e)),
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
        Err(e) => return Response::error(&format!("Invalid params: {}", e)),
    };

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    match google::delete_event(&params.google_account, calendar_id, &params.event_id).await {
        Ok(()) => Response::success(()),
        Err(e) => Response::error(&format!("{:#}", e)),
    }
}
