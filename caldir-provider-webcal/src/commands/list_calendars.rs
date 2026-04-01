//! List the single calendar represented by a webcal subscription.

use anyhow::Result;
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{Remote, protocol::ListCalendars, provider::Provider};

use crate::constants::PROVIDER_NAME;
use crate::remote_config::WebcalRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    // The account_identifier IS the URL for webcal subscriptions
    let session = Session::load(&cmd.account_identifier)?;

    // Fall back to the URL host for the display name
    let name = session.display_name.clone().unwrap_or_else(|| {
        url::Url::parse(&session.url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "Webcal".to_string())
    });

    let remote_config = WebcalRemoteConfig::new(&session.url);
    let remote = Remote::new(Provider::from_name(PROVIDER_NAME), remote_config.into());

    let config = CalendarConfig {
        name: Some(name),
        color: session.color.clone(),
        read_only: Some(true),
        remote: Some(remote),
    };

    Ok(vec![config])
}
