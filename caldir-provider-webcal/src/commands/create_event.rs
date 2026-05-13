//! Create event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::CreateEvent;

pub async fn handle(_cmd: CreateEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
