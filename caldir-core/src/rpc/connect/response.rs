use crate::CalendarConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectStepKind {
    /// OAuth flow (Google, Outlook...)
    OAuthRedirect,
    /// Hosted OAuth flow (via caldir.org)
    HostedOAuth,
    /// Form-based credentials (iCloud app password, CalDAV)
    Credentials,
    /// Provider needs one-time setup before auth can proceed.
    NeedsSetup,
}

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
    ///
    /// Multi-calendar account providers (Google, iCloud, Outlook, CalDAV) return
    /// just `account_identifier`; the CLI then calls `list_calendars` with it.
    ///
    /// Single-calendar providers (webcal) skip `list_calendars` entirely and
    /// return the calendar in `calendars` directly. They leave `account_identifier`
    /// empty since there's no account concept.
    Done {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_identifier: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        calendars: Option<Vec<CalendarConfig>>,
    },
}
