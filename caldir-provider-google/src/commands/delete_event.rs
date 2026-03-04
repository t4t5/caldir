use anyhow::{Context, Result};
use caldir_core::remote::protocol::DeleteEvent;
use google_calendar::types::SendUpdates;

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let google_event_id = cmd
        .event
        .custom_properties
        .iter()
        .find(|(k, _)| k == PROVIDER_EVENT_ID_PROPERTY)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot delete event without {PROVIDER_EVENT_ID_PROPERTY}"))?;

    let client = Session::load_valid(account_email).await?.client()?;

    client
        .events()
        .delete(calendar_id, google_event_id, false, SendUpdates::None)
        .await
        .context("Failed to delete event")?;

    Ok(())
}
