//! Scaffolding for caldir providers
//!
//! Providers implement [`ProviderHandler`].
//! It takes care of reading JSON payloads from stdin
//! and writing JSON responses to stdout.

use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::error::Error as StdError;
use std::future::Future;
use std::io::{self, BufRead, Write};

use super::{
    Connect, ConnectResponse, CreateEvent, DeleteEvent, ListCalendars, ListEvents, Method, Request,
    Response, UpdateEvent,
};
use crate::{CalendarConfig, Event};

/// Handles JSON errors from provider bins
pub type HandlerError = Box<dyn StdError + Send + Sync>;

pub type HandlerResult<T> = Result<T, HandlerError>;

/// Implemented by each provider to serve the caldir RPC protocol.
#[async_trait]
pub trait ProviderHandler: Send + Sync {
    async fn connect(&self, cmd: Connect) -> HandlerResult<ConnectResponse>;

    async fn list_calendars(&self, _cmd: ListCalendars) -> HandlerResult<Vec<CalendarConfig>> {
        Err("list_calendars is not supported by this provider".into())
    }

    async fn list_events(&self, _cmd: ListEvents) -> HandlerResult<Vec<Event>> {
        Err("list_events is not supported by this provider".into())
    }

    async fn create_event(&self, _cmd: CreateEvent) -> HandlerResult<Event> {
        Err("This provider does not support creating events".into())
    }

    async fn update_event(&self, _cmd: UpdateEvent) -> HandlerResult<Event> {
        Err("This provider does not support updating events".into())
    }

    async fn delete_event(&self, _cmd: DeleteEvent) -> HandlerResult<Event> {
        Err("This provider does not support deleting events".into())
    }
}

/// Run a provider as a subprocess speaking the caldir RPC protocol over
/// stdin/stdout. Blocks until stdin closes.
pub async fn run_provider<H: ProviderHandler>(handler: H) {
    let input = io::stdin().lock();
    let mut output = io::stdout();

    for line in input.lines() {
        let Ok(line) = line else { break };

        if line.trim().is_empty() {
            continue;
        }

        let response = process_request(&handler, &line).await;

        if writeln!(output, "{}", response).is_err() || output.flush().is_err() {
            break;
        }
    }
}

/// Process a single JSON-encoded request line and return the JSON-encoded
/// response. Exposed for unit tests — most providers only need [`run_provider`].
pub async fn process_request<H: ProviderHandler>(handler: &H, line: &str) -> String {
    let request: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return Response::error(&format!("Failed to parse request: {e}")),
    };

    match dispatch(handler, request).await {
        Ok(data) => Response::success(data),
        Err(e) => Response::error(&format!("Error handling request: {}", format_chain(&*e))),
    }
}

/// Preserves context from providers' `anyhow::Context`
fn format_chain(err: &(dyn StdError + 'static)) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(e) = source {
        out.push_str(": ");
        out.push_str(&e.to_string());
        source = e.source();
    }
    out
}

async fn dispatch<H: ProviderHandler>(
    handler: &H,
    request: Request,
) -> HandlerResult<serde_json::Value> {
    let Request { method, params } = request;

    match method {
        Method::Connect => call(params, |c| handler.connect(c)).await,
        Method::ListCalendars => call(params, |c| handler.list_calendars(c)).await,
        Method::ListEvents => call(params, |c| handler.list_events(c)).await,
        Method::CreateEvent => call(params, |c| handler.create_event(c)).await,
        Method::UpdateEvent => call(params, |c| handler.update_event(c)).await,
        Method::DeleteEvent => call(params, |c| handler.delete_event(c)).await,
    }
}

async fn call<C, R, F, Fut>(
    params: serde_json::Value,
    handler: F,
) -> HandlerResult<serde_json::Value>
where
    C: DeserializeOwned,
    R: Serialize,
    F: FnOnce(C) -> Fut,
    Fut: Future<Output = HandlerResult<R>>,
{
    let cmd: C = serde_json::from_value(params)?;
    let response = handler(cmd).await?;
    Ok(serde_json::to_value(response)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubHandler;

    #[async_trait]
    impl ProviderHandler for StubHandler {
        async fn connect(&self, _cmd: Connect) -> HandlerResult<ConnectResponse> {
            Ok(ConnectResponse::Done {
                account_identifier: Some("me@example.com".to_string()),
                calendars: None,
            })
        }
    }

    #[tokio::test]
    async fn dispatches_connect_and_returns_success_envelope() {
        let response = process_request(
            &StubHandler,
            r#"{"command":"connect","params":{"options":{},"data":{}}}"#,
        )
        .await;

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["data"]["account_identifier"], "me@example.com");
    }

    #[tokio::test]
    async fn unimplemented_method_returns_error_envelope() {
        let response = process_request(
            &StubHandler,
            r#"{"command":"list_calendars","params":{"account_identifier":"me@example.com"}}"#,
        )
        .await;

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["status"], "error");
        assert!(
            parsed["error"]
                .as_str()
                .unwrap()
                .contains("list_calendars is not supported"),
            "got: {}",
            parsed["error"]
        );
    }

    #[tokio::test]
    async fn error_response_includes_source_chain() {
        #[derive(Debug)]
        struct Outer;
        impl std::fmt::Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "outer")
            }
        }
        impl StdError for Outer {
            fn source(&self) -> Option<&(dyn StdError + 'static)> {
                Some(&Inner)
            }
        }

        #[derive(Debug)]
        struct Inner;
        impl std::fmt::Display for Inner {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "inner")
            }
        }
        impl StdError for Inner {}

        struct ChainHandler;
        #[async_trait]
        impl ProviderHandler for ChainHandler {
            async fn connect(&self, _cmd: Connect) -> HandlerResult<ConnectResponse> {
                Err(Box::new(Outer))
            }
        }

        let response = process_request(
            &ChainHandler,
            r#"{"command":"connect","params":{"options":{},"data":{}}}"#,
        )
        .await;

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["status"], "error");
        assert_eq!(parsed["error"], "Error handling request: outer: inner");
    }

    #[tokio::test]
    async fn malformed_json_returns_parse_error() {
        let response = process_request(&StubHandler, "not json").await;

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["status"], "error");
        assert!(
            parsed["error"]
                .as_str()
                .unwrap()
                .contains("Failed to parse request"),
        );
    }
}
