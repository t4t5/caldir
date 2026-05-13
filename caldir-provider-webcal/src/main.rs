//! Webcal (ICS subscription) provider for caldir.
//!
//! This binary implements the caldir provider protocol, communicating
//! with caldir-cli via JSON over stdin/stdout. It provides read-only
//! access to ICS calendar subscriptions (webcal:// feeds).

mod commands;
mod constants;
mod remote_config;

use anyhow::Result;
use caldir_core::rpc::{
    Connect, CreateEvent, DeleteEvent, ListCalendars, ListEvents, Method, Request, Response,
    UpdateEvent,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
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

async fn dispatch<C, R, F, Fut>(params: serde_json::Value, handler: F) -> Result<serde_json::Value>
where
    C: DeserializeOwned,
    R: Serialize,
    F: FnOnce(C) -> Fut,
    Fut: Future<Output = Result<R>>,
{
    let cmd: C = serde_json::from_value(params)?;
    let response = handler(cmd).await?;
    Ok(serde_json::to_value(response)?)
}

async fn handle_request(request: Request) -> Result<serde_json::Value> {
    let Request { method, params } = request;

    match method {
        Method::Connect => dispatch::<Connect, _, _, _>(params, commands::connect::handle).await,
        Method::ListCalendars => {
            dispatch::<ListCalendars, _, _, _>(params, commands::list_calendars::handle).await
        }
        Method::ListEvents => {
            dispatch::<ListEvents, _, _, _>(params, commands::list_events::handle).await
        }
        Method::CreateEvent => {
            dispatch::<CreateEvent, _, _, _>(params, commands::create_event::handle).await
        }
        Method::UpdateEvent => {
            dispatch::<UpdateEvent, _, _, _>(params, commands::update_event::handle).await
        }
        Method::DeleteEvent => {
            dispatch::<DeleteEvent, _, _, _>(params, commands::delete_event::handle).await
        }
    }
}
