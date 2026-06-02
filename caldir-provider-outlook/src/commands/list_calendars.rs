use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListCalendars;
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};

use crate::app_config::AppConfigStore;
use crate::constants::PROVIDER_NAME;
use crate::graph_api::client::GraphClient;
use crate::graph_api::types::{GraphCalendar, GraphResponse};
use crate::remote_config::OutlookRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let account_email = &cmd.account_identifier;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;
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
            let params =
                OutlookRemoteConfig::new(account_email, &cal.id).into_remote_config_params();
            let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);
            let read_only = !cal.can_edit;
            let color = graph_color_to_hex(&cal.color);

            CalendarConfig::new(
                Some(cal.name.clone()),
                Some(color),
                Some(read_only),
                Some(remote_config),
            )
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
