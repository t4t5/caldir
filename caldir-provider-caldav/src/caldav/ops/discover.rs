//! CalDAV endpoint discovery (principal + calendar home).

use anyhow::{Context, Result};
use http::Uri;
use libdav::caldav::FindCalendarHomeSet;

use crate::caldav::{
    CalDavClient_, absolute_url, create_caldav_client, find_well_known_context_url,
};

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
    let mut caldav = create_caldav_client(base_url, username, password)?;
    let mut principal = find_principal(&caldav).await;

    // Servers like Fastmail serve nothing at the URL they hand users, nor at the root,
    // and only advertise their context path through the well-known URI (RFC 6764).
    if !matches!(principal, Ok(Some(_)))
        && let Ok(Some(context_url)) =
            find_well_known_context_url(base_url, username, password).await
    {
        let bootstrapped = create_caldav_client(&context_url, username, password)?;
        let found = find_principal(&bootstrapped).await;
        if matches!(found, Ok(Some(_))) {
            caldav = bootstrapped;
            principal = found;
        }
    }

    let principal = principal?.ok_or_else(|| {
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

async fn find_principal(caldav: &CalDavClient_) -> Result<Option<Uri>> {
    caldav
        .find_current_user_principal()
        .await
        .context("Failed to find current user principal")
}
