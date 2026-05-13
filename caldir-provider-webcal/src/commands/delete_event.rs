//! Delete event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::DeleteEvent;

pub async fn handle(_cmd: DeleteEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
