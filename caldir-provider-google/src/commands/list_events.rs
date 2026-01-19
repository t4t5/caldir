use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::ListEvents;
use google_calendar::types::OrderBy;

use crate::google_event::FromGoogle;
use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote_config)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let client = Session::load_valid(account_email).await?.client()?;

    let google_events = client
        .events()
        .list_all(
            calendar_id,
            "",
            0,
            OrderBy::default(),
            &[],
            "", // search query
            &[],
            false,
            false,
            false,
            &cmd.to,
            &cmd.from,
            "",
            "",
        )
        .await
        .context("Failed to fetch events")?
        .body;

    let events: Vec<Event> = google_events
        .into_iter()
        .map(Event::from_google)
        .collect::<Result<_, _>>()?;

    Ok(events)
}
