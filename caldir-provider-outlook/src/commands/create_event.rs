use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::CreateEvent;

use crate::graph_client::GraphClient;
use crate::graph_types::GraphEvent;
use crate::outlook_event::from_outlook::from_outlook;
use crate::outlook_event::to_outlook::to_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    let body = to_outlook(&cmd.event);

    let path = format!("/me/calendars/{}/events", config.outlook_calendar_id);
    let response = graph
        .post(&path, &body)
        .await
        .with_context(|| format!("Failed to create event: {}", cmd.event.summary))?;

    let created: GraphEvent = response
        .json()
        .await
        .context("Failed to parse created event response")?;

    from_outlook(created, &config.outlook_account)
}
