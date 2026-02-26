//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout.

mod app_config;
mod commands;
mod constants;
mod google_event;
mod remote_config;
mod session;

use anyhow::Result;
use caldir_core::remote::protocol::{
    Command, Connect, CreateEvent, DeleteEvent, ListCalendars, ListEvents, ProviderCommand,
    Request, Response, UpdateEvent,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
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

/// Dispatch a command to its handler with compile-time type safety.
///
/// This ensures the handler returns the correct type as specified by
/// the command's `ProviderCommand::Response` associated type.
async fn dispatch<C, F, Fut>(params: serde_json::Value, handler: F) -> Result<serde_json::Value>
where
    C: ProviderCommand + DeserializeOwned,
    C::Response: Serialize,
    F: FnOnce(C) -> Fut,
    Fut: Future<Output = Result<C::Response>>,
{
    let cmd: C = serde_json::from_value(params)?;
    let response = handler(cmd).await?;
    Ok(serde_json::to_value(response)?)
}

async fn handle_request(request: Request) -> Result<serde_json::Value> {
    match request.command {
        Command::Connect => {
            dispatch::<Connect, _, _>(request.params, commands::connect::handle).await
        }
        Command::ListCalendars => {
            dispatch::<ListCalendars, _, _>(request.params, commands::list_calendars::handle).await
        }
        Command::ListEvents => {
            dispatch::<ListEvents, _, _>(request.params, commands::list_events::handle).await
        }
        Command::CreateEvent => {
            dispatch::<CreateEvent, _, _>(request.params, commands::create_event::handle).await
        }
        Command::UpdateEvent => {
            dispatch::<UpdateEvent, _, _>(request.params, commands::update_event::handle).await
        }
        Command::DeleteEvent => {
            dispatch::<DeleteEvent, _, _>(request.params, commands::delete_event::handle).await
        }
    }
}
