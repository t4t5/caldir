//! Complete authentication - validates credentials and discovers CalDAV endpoints.
//!
//! iCloud CalDAV discovery flow:
//! 1. Create CalDavClient pointing to caldav.icloud.com
//! 2. Use find_current_user_principal() to discover principal URL
//! 3. Use FindCalendarHomeSet to discover calendar home URL
//! 4. Save credentials and discovered URLs for future use

use anyhow::{Context, Result};
use caldir_core::remote::protocol::AuthSubmit;
use libdav::caldav::FindCalendarHomeSet;

use crate::caldav::create_caldav_client;
use crate::constants::CALDAV_ENDPOINT;
use crate::session::Session;

pub async fn handle(cmd: AuthSubmit) -> Result<String> {
    let apple_id = cmd
        .credentials
        .get("apple_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'apple_id' in credentials"))?;

    let app_password = cmd
        .credentials
        .get("app_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'app_password' in credentials"))?;

    // Discover CalDAV endpoints using libdav
    let (principal_url, calendar_home_url) =
        discover_caldav_endpoints(apple_id, app_password).await?;

    // Save session
    let session = Session::new(apple_id, app_password, &principal_url, &calendar_home_url);
    session.save()?;

    Ok(apple_id.to_string())
}

/// Discover CalDAV principal and calendar-home URLs using libdav.
async fn discover_caldav_endpoints(apple_id: &str, app_password: &str) -> Result<(String, String)> {
    // Create CalDAV client pointing to iCloud endpoint
    let caldav = create_caldav_client(CALDAV_ENDPOINT, apple_id, app_password)?;

    // Step 1: Find the user's principal URL
    let principal = caldav
        .find_current_user_principal()
        .await
        .context("Failed to find current user principal")?
        .ok_or_else(|| anyhow::anyhow!(
            "iCloud authentication failed. Check your Apple ID and app password."
        ))?;

    // Build absolute principal URL from the base URL
    let principal_url = format!(
        "{}://{}{}",
        caldav.base_url().scheme_str().unwrap_or("https"),
        caldav
            .base_url()
            .authority()
            .map(|a| a.as_str())
            .unwrap_or("caldav.icloud.com"),
        principal.path()
    );

    // Step 2: Find the calendar home set
    let home_set_response = caldav
        .request(FindCalendarHomeSet::new(&principal))
        .await
        .context("Failed to find calendar home set")?;

    let calendar_home = home_set_response
        .home_sets
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No calendar home set found for this account"))?;

    // Build absolute calendar home URL
    let calendar_home_url = format!(
        "{}://{}{}",
        caldav.base_url().scheme_str().unwrap_or("https"),
        caldav
            .base_url()
            .authority()
            .map(|a| a.as_str())
            .unwrap_or("caldav.icloud.com"),
        calendar_home.path()
    );

    Ok((principal_url, calendar_home_url))
}
