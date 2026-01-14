//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.

mod app_config;
mod commands;
mod convert;
mod session;

use anyhow::Result;
use caldir_core::protocol::{Command, Request, Response};
use std::io::{self, BufRead, Write};

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
        Command::Authenticate => commands::authenticate::handle().await,
        Command::ListCalendars => commands::list_calendars::handle(&request.params).await,
        Command::ListEvents => commands::list_events::handle(&request.params).await,
        Command::CreateEvent => commands::create_event::handle(&request.params).await,
        Command::UpdateEvent => commands::update_event::handle(&request.params).await,
        Command::DeleteEvent => commands::delete_event::handle(&request.params).await,
    }
}
