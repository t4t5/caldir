//! Webcal (ICS subscription) provider for caldir.
//!
//! webcal subscriptions are readonly, so they only implement "connect" and "list_events"

mod commands;
mod constants;
mod http;
mod remote_config;

use async_trait::async_trait;
use caldir_core::Event;
use caldir_core::rpc::{Connect, ConnectResponse, HandlerResult, ListEvents, ProviderHandler};

struct WebcalProvider;

#[async_trait]
impl ProviderHandler for WebcalProvider {
    async fn connect(&self, cmd: Connect) -> HandlerResult<ConnectResponse> {
        Ok(commands::connect::handle(cmd).await?)
    }

    async fn list_events(&self, cmd: ListEvents) -> HandlerResult<Vec<Event>> {
        Ok(commands::list_events::handle(cmd).await?)
    }
}

#[tokio::main]
async fn main() {
    caldir_core::rpc::run_provider(WebcalProvider).await
}
