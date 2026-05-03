//! Create event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::{CreateEvent, ProviderRequestContext};

pub async fn handle(_context: ProviderRequestContext, _cmd: CreateEvent) -> Result<Event> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
