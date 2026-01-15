//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.

mod app_config;
mod commands;
mod google_event;
mod session;

use anyhow::Result;
use caldir_core::protocol::{Command, Request, Response};
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() {
    let input = io::stdin().lock();
    let mut output = io::stdout();

    for line in input.lines() {
        let Ok(line) = line else { break };

        if line.trim().is_empty() {
            continue;
        }

        let response = process_request(&line).await;

        if writeln!(output, "{}", response).is_err() || output.flush().is_err() {
            break;
        }
    }
}

async fn process_request(line: &str) -> String {
    let request: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return Response::error(&format!("Failed to parse request: {e}")),
    };

    match handle_request(request).await {
        Ok(data) => Response::success(data),
        Err(e) => Response::error(&format!("Error handling request: {e:#}")),
    }
}

async fn handle_request(request: Request) -> Result<serde_json::Value> {
    match request.command {
        Command::Authenticate => commands::authenticate::handle(request.params).await,
        Command::ListCalendars => commands::list_calendars::handle(request.params).await,
        Command::ListEvents => commands::list_events::handle(request.params).await,
        Command::CreateEvent => commands::create_event::handle(request.params).await,
        Command::UpdateEvent => commands::update_event::handle(request.params).await,
        Command::DeleteEvent => commands::delete_event::handle(request.params).await,
    }
}
