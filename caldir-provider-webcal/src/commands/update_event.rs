//! Update event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::UpdateEvent;

pub async fn handle(_cmd: UpdateEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
