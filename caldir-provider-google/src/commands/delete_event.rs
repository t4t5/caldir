use anyhow::{Context, Result};
use caldir_core::remote::protocol::DeleteEvent;
use google_calendar::types::SendUpdates;

use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    client
        .events()
        .delete(calendar_id, &cmd.event_id, false, SendUpdates::None)
        .await
        .context("Failed to delete event")?;

    Ok(())
}
