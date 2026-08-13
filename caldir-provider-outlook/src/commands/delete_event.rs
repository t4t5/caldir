use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::DeleteEvent;

use crate::app_config::AppConfigStore;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
use crate::graph_api::client::GraphClient;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote)?;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(&config.outlook_account, &app_config_store)
        .await?;
    let graph = GraphClient::new(session.access_token());

    let outlook_event_id = cmd
        .event
        .x_property(PROVIDER_EVENT_ID_PROPERTY)
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
