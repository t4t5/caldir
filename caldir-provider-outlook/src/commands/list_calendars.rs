use anyhow::{Context, Result};
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::protocol::ListCalendars;
use caldir_core::remote::provider::Provider;
use caldir_core::remote::Remote;

use crate::graph_client::GraphClient;
use crate::graph_types::{GraphCalendar, GraphResponse};
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let account_email = &cmd.account_identifier;
    let session = Session::load_valid(account_email).await?;
    let graph = GraphClient::new(session.access_token());

    let response = graph.get("/me/calendars").await?;
    let calendars: GraphResponse<GraphCalendar> = response
        .json()
        .await
        .context("Failed to parse calendars response")?;

    let calendar_configs = calendars
        .value
        .iter()
        .map(|cal| {
            let remote_config = OutlookRemoteConfig::new(account_email, &cal.id);
            let remote = Remote::new(Provider::from_name("outlook"), remote_config.into());
            let read_only = !cal.can_edit;

            // Map Graph color names to hex colors
            let color = graph_color_to_hex(&cal.color);

            CalendarConfig {
                name: Some(cal.name.clone()),
                color: Some(color),
                read_only: Some(read_only),
                remote: Some(remote),
            }
        })
        .collect();

    Ok(calendar_configs)
}

fn graph_color_to_hex(color: &str) -> String {
    match color {
        "auto" | "lightBlue" => "#4285f4",
        "lightGreen" => "#0b8043",
        "lightOrange" => "#f4511e",
        "lightGray" => "#616161",
        "lightYellow" => "#e4c441",
        "lightTeal" => "#009688",
        "lightPink" => "#d81b60",
        "lightBrown" => "#795548",
        "lightRed" => "#d50000",
        "maxColor" => "#4285f4",
        _ => "#4285f4",
    }
    .to_string()
}
