use anyhow::{Context, Result};
use caldir_core::remote::protocol::DeleteEvent;

use crate::graph_client::GraphClient;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    let path = format!("/me/events/{}", cmd.event_id);
    graph
        .delete(&path)
        .await
        .context("Failed to delete event")?;

    Ok(())
}
