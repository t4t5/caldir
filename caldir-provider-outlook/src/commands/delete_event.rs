use anyhow::{Context, Result};
use caldir_core::remote::protocol::DeleteEvent;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::graph_api::client::GraphClient;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    let outlook_event_id = cmd
        .event
        .custom_properties
        .iter()
        .find(|(k, _)| k == PROVIDER_EVENT_ID_PROPERTY)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot delete event without {PROVIDER_EVENT_ID_PROPERTY}")
        })?;

    let path = format!("/me/events/{}", outlook_event_id);
    graph
        .delete(&path)
        .await
        .context("Failed to delete event")?;

    Ok(())
}
