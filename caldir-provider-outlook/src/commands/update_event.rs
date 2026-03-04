use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::UpdateEvent;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::graph_client::GraphClient;
use crate::graph_types::GraphEvent;
use crate::outlook_event::from_outlook::from_outlook;
use crate::outlook_event::to_outlook::to_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    let outlook_event_id = cmd
        .event
        .custom_properties
        .iter()
        .find(|(k, _)| k == PROVIDER_EVENT_ID_PROPERTY)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot update event without {PROVIDER_EVENT_ID_PROPERTY}"))?;

    let body = to_outlook(&cmd.event);

    let path = format!("/me/events/{}", outlook_event_id);
    let response = graph.patch(&path, &body).await?;

    let updated: GraphEvent = response
        .json()
        .await?;

    from_outlook(updated)
}
