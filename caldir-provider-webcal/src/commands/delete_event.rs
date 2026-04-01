//! Delete event is not supported for webcal subscriptions.

use anyhow::Result;
use caldir_core::remote::protocol::DeleteEvent;

pub async fn handle(_cmd: DeleteEvent) -> Result<()> {
    anyhow::bail!("This calendar is read-only (webcal subscription)")
}
