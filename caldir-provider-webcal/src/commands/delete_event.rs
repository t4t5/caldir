//! Delete event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::remote::protocol::{DeleteEvent, ProviderRequestContext};

pub async fn handle(_context: ProviderRequestContext, _cmd: DeleteEvent) -> Result<()> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
