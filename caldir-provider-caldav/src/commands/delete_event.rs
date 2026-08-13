//! Delete an event from a CalDAV calendar.

use anyhow::Result;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::DeleteEvent;
use caldir_provider_caldav::caldav::ops;

use crate::constants::PROVIDER_NAME;
use crate::remote_config::CaldavRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let store = SessionStore::new(ProviderStorage::for_provider(PROVIDER_NAME)?);
    let session = store.load(&config.caldav_account)?;
    let (username, password) = session.credentials();

    ops::delete_event(
        username,
        password,
        &config.caldav_calendar_url,
        cmd.event.uid.as_str(),
    )
    .await
}
