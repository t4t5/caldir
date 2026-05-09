use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Op {
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) trait ProviderCommand: Serialize {
    type Response: DeserializeOwned;
    const OP: Op;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ProviderRequest {
    pub(crate) op: Op,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ProviderResponse<T> {
    Success { data: T },
    Error { error: String },
}
