//! Update event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::{ProviderRequestContext, UpdateEvent};

pub async fn handle(_context: ProviderRequestContext, _cmd: UpdateEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
