//! Provider protocol types.
//!
//! Defines the JSON protocol used for communication between caldir-cli
//! and provider binaries over stdin/stdout.

use serde::{Deserialize, Serialize};

/// Commands that providers must implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Authenticate,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

/// Request sent from CLI to provider.
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub command: Command,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Response sent from provider to CLI.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response<T> {
    Success { data: T },
    Error { error: String },
}

impl<T: Serialize> Response<T> {
    pub fn success(data: T) -> String {
        serde_json::to_string(&Response::Success { data }).unwrap()
    }
}

impl Response<()> {
    pub fn error(msg: &str) -> String {
        serde_json::to_string(&Response::<()>::Error {
            error: msg.to_string(),
        })
        .unwrap()
    }
}
