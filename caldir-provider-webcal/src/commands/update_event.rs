//! Update event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::UpdateEvent;

pub async fn handle(_cmd: UpdateEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
