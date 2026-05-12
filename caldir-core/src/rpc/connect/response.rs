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
    Done { account_identifier: String },
}
