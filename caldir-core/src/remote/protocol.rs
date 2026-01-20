//! Defines the JSON protocol used for communication between caldir-cli
//! and provider binaries over stdin/stdout.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{calendar_config::CalendarConfig, event::Event};

pub trait ProviderCommand: Serialize {
    type Response: DeserializeOwned;
    fn command() -> Command;
}

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

/// Authenticate with a provider and return the account identifier.
#[derive(Debug, Serialize, Deserialize)]
pub struct Authenticate {}

impl ProviderCommand for Authenticate {
    type Response = String; // Account identifier (e.g., email)
    fn command() -> Command {
        Command::Authenticate
    }
}

/// List all calendars for an authenticated account.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListCalendars {
    pub account_identifier: String,
}

impl ProviderCommand for ListCalendars {
    type Response = Vec<CalendarConfig>;
    fn command() -> Command {
        Command::ListCalendars
    }
}

/// List events within a time range.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListEvents {
    /// Provider-specific config (e.g., google_account, google_calendar_id)
    #[serde(flatten)]
    pub remote_config: serde_json::Map<String, serde_json::Value>,
    pub from: String,
    pub to: String,
}

impl ProviderCommand for ListEvents {
    type Response = Vec<Event>;
    fn command() -> Command {
        Command::ListEvents
    }
}

/// Create a new event.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEvent {
    #[serde(flatten)]
    pub remote_config: serde_json::Map<String, serde_json::Value>,
    pub event: Event,
}

impl ProviderCommand for CreateEvent {
    type Response = Event;
    fn command() -> Command {
        Command::CreateEvent
    }
}

/// Update an existing event.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEvent {
    #[serde(flatten)]
    pub remote_config: serde_json::Map<String, serde_json::Value>,
    pub event: Event,
}

impl ProviderCommand for UpdateEvent {
    type Response = Event;
    fn command() -> Command {
        Command::UpdateEvent
    }
}

/// Delete an event by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteEvent {
    #[serde(flatten)]
    pub remote_config: serde_json::Map<String, serde_json::Value>,
    pub event_id: String,
}

impl ProviderCommand for DeleteEvent {
    type Response = ();
    fn command() -> Command {
        Command::DeleteEvent
    }
}
