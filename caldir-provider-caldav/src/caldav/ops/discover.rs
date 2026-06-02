//! CalDAV endpoint discovery (principal + calendar home).

use anyhow::{Context, Result};
use libdav::caldav::FindCalendarHomeSet;

use crate::caldav::{absolute_url, create_caldav_client};

/// Discovered CalDAV endpoints from the connect flow.
pub struct DiscoveredEndpoints {
    pub principal_url: String,
    pub calendar_home_url: String,
}

/// Discover CalDAV principal and calendar-home URLs.
///
/// Performs PROPFIND requests to find the current user principal and calendar home set.
pub async fn discover_endpoints(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<DiscoveredEndpoints> {
    let caldav = create_caldav_client(base_url, username, password)?;

    let principal = caldav
        .find_current_user_principal()
        .await
        .context("Failed to find current user principal")?
        .ok_or_else(|| {
            anyhow::anyhow!("Authentication failed. Check your username and password.")
        })?;

    let principal_url = absolute_url(&caldav, principal.path());

    let home_set_response = caldav
        .request(FindCalendarHomeSet::new(principal.path()))
        .await
        .context("Failed to find calendar home set")?;

    let calendar_home = home_set_response
        .home_sets
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No calendar home set found for this account"))?;

    let calendar_home_url = absolute_url(&caldav, calendar_home.path());

    Ok(DiscoveredEndpoints {
        principal_url,
        calendar_home_url,
    })
}
