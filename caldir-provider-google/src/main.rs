//! caldir-provider-google - Google Calendar provider for caldir-cli
//!
//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.
//!
//! The provider manages its own credentials and tokens:
//!   ~/.config/caldir/providers/google/credentials.json
//!   ~/.config/caldir/providers/google/tokens/{account}.json

mod commands;
mod config;
mod google_auth;
mod parser;
mod types;

use anyhow::Result;
use caldir_core::protocol::{Command, Request, Response};
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
                if writeln!(stdout, "{}", response).is_err() || stdout.flush().is_err() {
                    break; // Parent likely killed us, exit silently
                }
                continue;
            }
        };

        let result = handle_request(request).await;

        let protocol_response = match result {
            Ok(data) => Response::success(data),
            Err(e) => Response::error(&format!("Error handling request: {:#}", e)),
        };

        if writeln!(stdout, "{}", protocol_response).is_err() || stdout.flush().is_err() {
            break; // Parent likely killed us, exit silently
        }
    }
}

async fn handle_request(request: Request) -> Result<serde_json::Value> {
    match request.command {
        Command::Authenticate => commands::handle_authenticate().await,
        Command::ListCalendars => commands::handle_list_calendars(&request.params).await,
        Command::ListEvents => commands::handle_list_events(&request.params).await,
        Command::CreateEvent => commands::handle_create_event(&request.params).await,
        Command::UpdateEvent => commands::handle_update_event(&request.params).await,
        Command::DeleteEvent => commands::handle_delete_event(&request.params).await,
    }
}
