mod ics;
mod methods;
mod wire;

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub use methods::{
    Connect, ConnectResponse, ConnectStepKind, CreateEvent, CredentialField, CredentialsData,
    DeleteEvent, FieldType, HostedOAuthData, ListCalendars, ListEvents, Method, OAuthData,
    SetupData, UpdateEvent,
};

pub(crate) use ics::Ics;
#[cfg(test)]
pub(crate) use wire::JsonWireValue;
pub(crate) use wire::{Wire, WireValue};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

// Handles serialization of command + deserialization of response
pub(crate) trait Rpc: Serialize {
    type Response: WireValue;
    const METHOD: Method;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;

    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error>
    where
        Self: Sized,
    {
        serde_json::to_value(Request::from_rpc(self)?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    #[serde(rename = "command")]
    pub method: Method,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl Request {
    pub(crate) fn from_rpc<C: Rpc>(cmd: &C) -> Result<Self, serde_json::Error> {
        Ok(Self {
            method: C::METHOD,
            params: serde_json::to_value(cmd)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response<T> {
    Success { data: T },
    Error { error: String },
}

impl<T: Serialize> Response<T> {
    /// Serialize a success response to a JSON string for stdout.
    pub fn success(data: T) -> String {
        serde_json::to_string(&Response::Success { data })
            .expect("Response::Success serialization is infallible for Serialize types")
    }
}

impl Response<()> {
    /// Serialize an error response to a JSON string for stdout.
    pub fn error(msg: &str) -> String {
        serde_json::to_string(&Response::<()>::Error {
            error: msg.to_string(),
        })
        .expect("Response::Error serialization is infallible")
    }
}
