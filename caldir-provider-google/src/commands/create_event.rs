use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::CreateEvent;
use google_calendar::types::SendUpdates;

use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    // Let google change the ID
    // (Otherwise we'll get "Invalid resource id value")
    let mut google_event = cmd.event.to_google();
    google_event.id = String::new();

    let response = client
        .events()
        .insert(
            calendar_id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to create event: {}", &google_event.summary))?;

    let created_event = Event::from_google(response.body)?;

    Ok(created_event)
}
