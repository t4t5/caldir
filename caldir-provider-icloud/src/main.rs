//! iCloud Calendar provider for caldir.

mod commands;
mod constants;
mod remote_config;
mod session;

use async_trait::async_trait;
use caldir_core::rpc::{
    Connect, ConnectResponse, CreateEvent, DeleteEvent, ListCalendars, ListEvents, UpdateEvent,
};
use caldir_core::{CalendarConfig, Event, Ics, provider};

struct ICloudProvider;

#[async_trait]
impl provider::Handler for ICloudProvider {
    async fn connect(&self, cmd: Connect) -> provider::Result<ConnectResponse> {
        Ok(commands::connect::handle(cmd).await?)
    }

    async fn list_calendars(&self, cmd: ListCalendars) -> provider::Result<Vec<CalendarConfig>> {
        Ok(commands::list_calendars::handle(cmd).await?)
    }

    async fn list_events(&self, cmd: ListEvents) -> provider::Result<Vec<Ics<Event>>> {
        Ok(commands::list_events::handle(cmd)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn create_event(&self, cmd: CreateEvent) -> provider::Result<Ics<Event>> {
        Ok(commands::create_event::handle(cmd).await?.into())
    }

    async fn update_event(&self, cmd: UpdateEvent) -> provider::Result<Ics<Event>> {
        Ok(commands::update_event::handle(cmd).await?.into())
    }

    async fn delete_event(&self, cmd: DeleteEvent) -> provider::Result<()> {
        Ok(commands::delete_event::handle(cmd).await?)
    }
}

#[tokio::main]
async fn main() {
    provider::run_provider(ICloudProvider).await
}
