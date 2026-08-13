//! Create a new event on iCloud Calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::CreateEvent;
use caldir_provider_caldav::caldav::ops;

use crate::constants::PROVIDER_NAME;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote)?;
    let store = SessionStore::new(ProviderStorage::for_provider(PROVIDER_NAME)?);
    let session = store.load(&config.icloud_account)?;
    let (username, password) = session.credentials();

    ops::create_event(username, password, &config.icloud_calendar_url, cmd.event).await
}
