//! Webcal is a single-calendar provider — its calendar is returned directly
//! from `connect`, so this RPC should never be called.

use anyhow::Result;
use caldir_core::CalendarConfig;
use caldir_core::rpc::ListCalendars;

pub async fn handle(_cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    anyhow::bail!(
        "list_calendars is not supported for webcal — the calendar is returned by `connect`"
    )
}
