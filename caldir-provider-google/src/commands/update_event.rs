use anyhow::Result;
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::UpdateEvent;
use google_calendar::types::SendUpdates;

use crate::app_config::AppConfigStore;
use crate::commands::invite::patch_invite_status;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
use crate::google_event::{FromGoogle, ToGoogle};
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = GoogleRemoteConfig::try_from(&cmd.remote)?;
    let account_email = &config.google_account;
    let calendar_id = &config.google_calendar_id;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;

    // Get Google's event ID from custom properties
    let google_event_id = cmd
        .event
        .x_property(PROVIDER_EVENT_ID_PROPERTY)
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot update event without {PROVIDER_EVENT_ID_PROPERTY}")
        })?;

    if cmd.event.is_invite_for(account_email) {
        // Only update our own attendee status:
        let google_event = patch_invite_status(
            &session,
            calendar_id,
            google_event_id,
            &cmd.event,
            account_email,
        )
        .await?;

        Ok(Event::from_google(google_event)?)
    } else {
        // Organizer or own event: full PUT update
        let client = session_store.client(&session, &app_config_store)?;

        let google_event = cmd.event.to_google();

        let response = client
            .events()
            .update(
                calendar_id,
                google_event_id,
                0,
                0,
                false,
                SendUpdates::All,
                false,
                &google_event,
            )
            .await?;

        Ok(Event::from_google(response.body)?)
    }
}
