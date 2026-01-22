//! Complete authentication - validates credentials and discovers CalDAV endpoints.
//!
//! iCloud CalDAV discovery flow:
//! 1. PROPFIND on caldav.icloud.com to get user's principal URL
//! 2. PROPFIND on principal to get calendar-home-set URL
//! 3. Save credentials and discovered URLs for future use

use anyhow::{Context, Result};
use caldir_core::remote::protocol::AuthSubmit;

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

    // Discover CalDAV endpoints using blocking HTTP
    let (principal_url, calendar_home_url) = tokio::task::spawn_blocking({
        let apple_id = apple_id.to_string();
        let app_password = app_password.to_string();
        move || discover_caldav_endpoints(&apple_id, &app_password)
    })
    .await
    .context("Task join error")??;

    // Save session
    let session = Session::new(apple_id, app_password, &principal_url, &calendar_home_url);
    session.save()?;

    Ok(apple_id.to_string())
}

/// Discover CalDAV principal and calendar-home URLs via PROPFIND requests.
fn discover_caldav_endpoints(apple_id: &str, app_password: &str) -> Result<(String, String)> {
    // Create client that follows redirects (iCloud redirects to user-specific server)
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Failed to create HTTP client")?;

    // Step 1: PROPFIND on well-known CalDAV endpoint to get principal
    let propfind_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:current-user-principal/>
  </d:prop>
</d:propfind>"#;

    let response = client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            format!("{}/", CALDAV_ENDPOINT),
        )
        .basic_auth(apple_id, Some(app_password))
        .header("Content-Type", "application/xml; charset=utf-8")
        .header("Depth", "0")
        .body(propfind_body)
        .send()
        .context("Failed to connect to iCloud CalDAV")?;

    let status = response.status();
    let final_url = response.url().clone();

    if !status.is_success() && status.as_u16() != 207 {
        anyhow::bail!(
            "iCloud authentication failed (status {}). Check your Apple ID and app password.",
            status
        );
    }

    let body = response.text().context("Failed to read response body")?;

    // Parse principal URL from response
    let principal_path = extract_href(&body, "current-user-principal")
        .with_context(|| format!("Could not find principal URL in response. Response body:\n{}", body))?;

    // Make the principal URL absolute using the final URL (after redirects)
    let base_url = get_base_url(&final_url);
    let principal_url = make_absolute_url(&principal_path, &base_url);

    // Step 2: PROPFIND on principal to get calendar-home-set
    let propfind_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <c:calendar-home-set/>
  </d:prop>
</d:propfind>"#;

    let response = client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            &principal_url,
        )
        .basic_auth(apple_id, Some(app_password))
        .header("Content-Type", "application/xml; charset=utf-8")
        .header("Depth", "0")
        .body(propfind_body)
        .send()
        .context("Failed to get calendar home")?;

    let status = response.status();
    let final_url = response.url().clone();

    if !status.is_success() && status.as_u16() != 207 {
        anyhow::bail!(
            "Failed to get calendar home (status {})",
            status
        );
    }

    let body = response.text().context("Failed to read response body")?;

    let calendar_home_path = extract_href(&body, "calendar-home-set")
        .with_context(|| format!("Could not find calendar-home-set in response. Response body:\n{}", body))?;

    // Make the calendar home URL absolute
    let base_url = get_base_url(&final_url);
    let calendar_home_url = make_absolute_url(&calendar_home_path, &base_url);

    Ok((principal_url, calendar_home_url))
}

/// Get base URL (scheme + host) from a URL.
fn get_base_url(url: &reqwest::Url) -> String {
    format!("{}://{}", url.scheme(), url.host_str().unwrap_or("caldav.icloud.com"))
}

/// Make a URL absolute if it's relative.
fn make_absolute_url(path: &str, base_url: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", base_url.trim_end_matches('/'), path)
    }
}

/// Extract href value from a DAV property in XML response.
/// Handles various XML namespace prefixes used by different CalDAV servers.
fn extract_href(xml: &str, property_name: &str) -> Option<String> {
    // Find the property (case-insensitive, handles various namespace prefixes)
    let xml_lower = xml.to_lowercase();

    // Look for the property with various possible prefixes
    let prop_patterns = [
        format!("<d:{}", property_name),
        format!("<D:{}", property_name),
        format!("<{}", property_name),
        format!(":{}",  property_name.to_lowercase()),
    ];

    let prop_start = prop_patterns
        .iter()
        .filter_map(|p| xml_lower.find(&p.to_lowercase()))
        .min()?;

    // Find the href within/after this property
    // Look for various href tag formats
    let remaining = &xml[prop_start..];

    let href_patterns = [
        ("<d:href>", "</d:href>"),
        ("<D:href>", "</D:href>"),
        ("<href>", "</href>"),
        ("<ns0:href>", "</ns0:href>"),
        ("<ns1:href>", "</ns1:href>"),
    ];

    for (start_tag, end_tag) in href_patterns {
        // Case-insensitive search
        let remaining_lower = remaining.to_lowercase();
        let start_tag_lower = start_tag.to_lowercase();
        let end_tag_lower = end_tag.to_lowercase();

        if let Some(href_start) = remaining_lower.find(&start_tag_lower) {
            let content_start = href_start + start_tag.len();
            let remaining_after = &remaining[content_start..];
            let remaining_after_lower = remaining_after.to_lowercase();

            if let Some(href_end) = remaining_after_lower.find(&end_tag_lower) {
                return Some(remaining_after[..href_end].trim().to_string());
            }
        }
    }

    // Fallback: try to find any href tag with regex-like pattern
    // Look for >...</ pattern after href
    if let Some(href_pos) = xml_lower[prop_start..].find("href") {
        let after_href = &xml[prop_start + href_pos..];
        if let Some(gt_pos) = after_href.find('>') {
            let content_start = gt_pos + 1;
            let after_gt = &after_href[content_start..];
            if let Some(lt_pos) = after_gt.find('<') {
                let href_value = after_gt[..lt_pos].trim();
                if !href_value.is_empty() {
                    return Some(href_value.to_string());
                }
            }
        }
    }

    None
}
