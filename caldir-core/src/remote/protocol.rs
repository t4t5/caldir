//! Defines the JSON protocol used for communication between caldir-cli
//! and provider binaries over stdin/stdout.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{calendar::config::CalendarConfig, event::Event};

pub trait ProviderCommand: Serialize {
    type Response: DeserializeOwned;
    fn command() -> Command;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    AuthInit,
    AuthSubmit,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

// ============================================================================
// Auth Types
// ============================================================================

/// The type of authentication the provider requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// OAuth 2.0 redirect flow (Google, Microsoft)
    OAuthRedirect,
    /// Form-based credentials (iCloud app password, CalDAV)
    Credentials,
}

/// OAuth-specific init response data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthData {
    pub authorization_url: String,
    pub state: String,
    pub scopes: Vec<String>,
}

/// Credentials-specific init response data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsData {
    pub fields: Vec<CredentialField>,
}

/// A field required for credentials-based authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialField {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// The type of input field for credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Password,
    Url,
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

/// Request to initialize authentication.
/// Provider returns what auth method it needs and any required data.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthInit {
    /// For OAuth flows: the redirect URI the caller will listen on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

/// Response from auth initialization - varies by auth type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInitResponse {
    pub auth_type: AuthType,
    /// Type-specific data (OAuthData or CredentialsData as JSON).
    #[serde(flatten)]
    pub data: serde_json::Value,
}

impl ProviderCommand for AuthInit {
    type Response = AuthInitResponse;
    fn command() -> Command {
        Command::AuthInit
    }
}

/// Submit gathered credentials to complete authentication.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSubmit {
    /// Gathered credentials - structure depends on auth_type.
    #[serde(flatten)]
    pub credentials: serde_json::Map<String, serde_json::Value>,
}

impl ProviderCommand for AuthSubmit {
    type Response = String; // Account identifier (e.g., email)
    fn command() -> Command {
        Command::AuthSubmit
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
