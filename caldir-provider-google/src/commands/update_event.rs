use anyhow::Result;
use caldir_core::Event;
use google_calendar::{Client, types::SendUpdates};
use serde::Deserialize;

use crate::{
    google::{from_google::from_google_event, to_google::to_google_event},
    types,
};

#[derive(Debug, Deserialize)]
struct UpdateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: types::Event,
}

pub async fn handle_update_event(
    client: &Client,
    calendar_id: &str,
    params: &serde_json::Value,
) -> Result<Event> {
    let params: UpdateEventParams = serde_json::from_value(params.clone())?;

    let event = params.event;

    let google_event = to_google_event(&event);

    let response = client
        .events()
        .update(
            calendar_id,
            &event.id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await?;

    from_google_event(response.body)
}
