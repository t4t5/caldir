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
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

// ============================================================================
// Connect Types
// ============================================================================

/// What the provider needs from the CLI in this step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectStepKind {
    /// OAuth 2.0 redirect flow (Google, Microsoft)
    OAuthRedirect,
    /// Hosted OAuth flow via caldir.org relay (no local client_id/secret needed)
    HostedOAuth,
    /// Form-based credentials (iCloud app password, CalDAV)
    Credentials,
    /// Provider needs one-time setup before auth can proceed.
    NeedsSetup,
}

/// OAuth-specific data for the OAuthRedirect step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthData {
    pub authorization_url: String,
    pub state: String,
    pub scopes: Vec<String>,
}

/// Hosted OAuth data for the HostedOAuth step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedOAuthData {
    pub url: String,
}

/// Credentials data for the Credentials step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsData {
    pub fields: Vec<CredentialField>,
}

/// Setup data for the NeedsSetup step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupData {
    pub instructions: String,
    pub fields: Vec<CredentialField>,
}

/// A field required for credentials or setup.
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

/// Request to advance the connect flow.
///
/// First call: `options` contains provider-specific hints (e.g., `redirect_uri`, `hosted`).
/// Subsequent calls: `data` contains gathered credentials/setup fields from the previous step.
#[derive(Debug, Serialize, Deserialize)]
pub struct Connect {
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

/// Response from the connect command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectResponse {
    /// Provider needs more input from the user.
    NeedsInput {
        step: ConnectStepKind,
        /// Step-specific data (OAuthData, CredentialsData, SetupData, etc.)
        #[serde(flatten)]
        data: serde_json::Value,
    },
    /// Connection complete.
    Done {
        account_identifier: String,
    },
}

impl ProviderCommand for Connect {
    type Response = ConnectResponse;
    fn command() -> Command {
        Command::Connect
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
