use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::UpdateEvent;
use google_calendar::types::SendUpdates;

use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    let google_event = cmd.event.to_google();

    // Get Google's event ID from custom properties
    let google_event_id = cmd
        .event
        .custom_properties
        .iter()
        .find(|(k, _)| k == "X-GOOGLE-EVENT-ID")
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot update event without X-GOOGLE-EVENT-ID"))?;

    let response = client
        .events()
        .update(
            calendar_id,
            google_event_id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await?;

    let updated_event = Event::from_google(response.body)?;
    Ok(updated_event)
}
