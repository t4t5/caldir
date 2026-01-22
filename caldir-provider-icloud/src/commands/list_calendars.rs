//! List iCloud Calendars (name + config) for a given account.
//!
//! Uses PROPFIND on the calendar-home-set to discover all calendar collections.

use anyhow::{Context, Result};
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{protocol::ListCalendars, provider::Provider, Remote};

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let apple_id = &cmd.account_identifier;

    let session = Session::load(apple_id)?;

    // List calendars using blocking HTTP (CalDAV is sync)
    let calendars = tokio::task::spawn_blocking({
        let session = session.clone();
        move || list_calendars_caldav(&session)
    })
    .await
    .context("Task join error")??;

    Ok(calendars)
}

/// Parsed calendar info from CalDAV PROPFIND response.
struct CalendarInfo {
    url: String,
    name: String,
    color: Option<String>,
}

/// List calendars via PROPFIND on calendar-home-set.
fn list_calendars_caldav(session: &Session) -> Result<Vec<CalendarConfig>> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Failed to create HTTP client")?;

    let propfind_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/" xmlns:ic="http://apple.com/ns/ical/">
  <d:prop>
    <d:displayname/>
    <d:resourcetype/>
    <ic:calendar-color/>
  </d:prop>
</d:propfind>"#;

    let (username, password) = session.credentials();

    let response = client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            &session.calendar_home_url,
        )
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/xml; charset=utf-8")
        .header("Depth", "1")
        .body(propfind_body)
        .send()
        .context("Failed to list calendars")?;

    let status = response.status();
    let final_url = response.url().clone();

    if !status.is_success() && status.as_u16() != 207 {
        anyhow::bail!(
            "Failed to list calendars (status {})",
            status
        );
    }

    let body = response.text().context("Failed to read response body")?;

    // Get base URL from the final URL (after any redirects)
    let base_url = format!("{}://{}", final_url.scheme(), final_url.host_str().unwrap_or("caldav.icloud.com"));

    // Parse calendar info from response
    let calendars = parse_calendar_list(&body, &session.calendar_home_url, &base_url);

    if calendars.is_empty() {
        // Return error with response body for debugging
        anyhow::bail!("No calendars found in response. Response body:\n{}", body);
    }

    // Convert to CalendarConfig
    let configs = calendars
        .into_iter()
        .map(|cal| {
            let remote_config = ICloudRemoteConfig::new(&session.apple_id, &cal.url);
            let remote = Remote::new(Provider::from_name("icloud"), remote_config.into());

            CalendarConfig {
                name: Some(cal.name),
                color: cal.color,
                remote: Some(remote),
            }
        })
        .collect();

    Ok(configs)
}

/// Parse calendar list from PROPFIND multistatus response.
fn parse_calendar_list(xml: &str, calendar_home_url: &str, base_url: &str) -> Vec<CalendarInfo> {
    let mut calendars = Vec::new();
    let xml_lower = xml.to_lowercase();

    // Find all response elements (handle various namespace prefixes)
    let response_starts: Vec<usize> = find_all_tag_starts(&xml_lower, "response");

    for start_idx in response_starts {
        // Find the end of this response element
        let response_end = find_closing_tag(&xml_lower[start_idx..], "response")
            .map(|end| start_idx + end)
            .unwrap_or(xml.len());

        let response = &xml[start_idx..response_end];
        let response_lower = response.to_lowercase();

        // Check if this is a calendar collection (resourcetype contains calendar)
        let is_calendar = response_lower.contains("calendar")
            && response_lower.contains("resourcetype");

        // Also check it's not just a collection without calendar type
        // (the calendar-home itself is a collection but not a calendar)
        let has_calendar_tag = response_lower.contains(":calendar")
            || response_lower.contains("<calendar");

        if !is_calendar || !has_calendar_tag {
            continue;
        }

        // Extract href
        let href = extract_tag_content(response, "href");
        let Some(href) = href else { continue };

        // Skip the calendar-home itself
        let href_normalized = href.trim_end_matches('/');
        let home_normalized = calendar_home_url
            .split("://")
            .nth(1)
            .and_then(|s| s.find('/').map(|i| &s[i..]))
            .unwrap_or(calendar_home_url)
            .trim_end_matches('/');

        // Check if this href is just the home URL (not a sub-calendar)
        if href_normalized == home_normalized || href_normalized.ends_with(home_normalized) {
            continue;
        }

        // Extract displayname
        let name = extract_tag_content(response, "displayname")
            .unwrap_or_else(|| {
                // Fallback: use last path segment as name
                href_normalized
                    .rsplit('/')
                    .next()
                    .unwrap_or("Calendar")
                    .to_string()
            });

        // Skip if name suggests this isn't a user calendar
        if name.is_empty() {
            continue;
        }

        // Extract color (Apple-specific property)
        let color = extract_tag_content(response, "calendar-color")
            .map(|c| {
                // iCloud returns colors as #RRGGBBAA, convert to #RRGGBB
                if c.len() == 9 && c.starts_with('#') {
                    c[..7].to_string()
                } else {
                    c
                }
            });

        // Build absolute URL
        let url = if href.starts_with("http") {
            href
        } else {
            format!("{}{}", base_url, href)
        };

        calendars.push(CalendarInfo { url, name, color });
    }

    calendars
}

/// Find all starting positions of a tag (handles various namespace prefixes).
fn find_all_tag_starts(xml_lower: &str, tag_name: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let patterns = [
        format!("<{}:", tag_name),      // <d:response, <D:response, etc.
        format!("<{}", tag_name),        // <response (no prefix)
    ];

    for pattern in &patterns {
        let mut search_start = 0;
        while let Some(pos) = xml_lower[search_start..].find(pattern.as_str()) {
            let absolute_pos = search_start + pos;
            // Make sure this is actually a tag start (followed by > or space or /)
            let after_pattern = &xml_lower[absolute_pos + pattern.len()..];
            if after_pattern.starts_with('>') || after_pattern.starts_with(' ') || after_pattern.starts_with('/') {
                positions.push(absolute_pos);
            }
            search_start = absolute_pos + 1;
        }
    }

    positions.sort();
    positions.dedup();
    positions
}

/// Find the closing tag position relative to the start.
fn find_closing_tag(xml_lower: &str, tag_name: &str) -> Option<usize> {
    let patterns = [
        format!("</{}:", tag_name),
        format!("</{}", tag_name),
    ];

    patterns
        .iter()
        .filter_map(|p| xml_lower.find(p.as_str()))
        .min()
        .map(|pos| {
            // Find the actual end of the closing tag
            xml_lower[pos..].find('>').map(|end| pos + end + 1).unwrap_or(pos)
        })
}

/// Extract content from a tag, handling various namespace prefixes.
fn extract_tag_content(xml: &str, tag_name: &str) -> Option<String> {
    let xml_lower = xml.to_lowercase();
    let tag_lower = tag_name.to_lowercase();

    // Try various patterns for opening tag
    let open_patterns = [
        format!("<d:{}>", tag_lower),
        format!("<d:{} ", tag_lower),  // tag with attributes
        format!("<{}:{}>", "d", tag_lower),
        format!("<{}>", tag_lower),
        format!("<ic:{}>", tag_lower),
        format!("<cs:{}>", tag_lower),
        format!("<c:{}>", tag_lower),
    ];

    for open_pattern in &open_patterns {
        if let Some(start_pos) = xml_lower.find(open_pattern.trim_end_matches('>')) {
            // Find the actual end of opening tag
            let tag_end = xml_lower[start_pos..].find('>')?;
            let content_start = start_pos + tag_end + 1;

            // Find closing tag
            let close_patterns = [
                format!("</d:{}>", tag_lower),
                format!("</{}>", tag_lower),
                format!("</ic:{}>", tag_lower),
                format!("</cs:{}>", tag_lower),
                format!("</c:{}>", tag_lower),
            ];

            for close_pattern in &close_patterns {
                if let Some(end_pos) = xml_lower[content_start..].find(close_pattern.as_str()) {
                    let content = &xml[content_start..content_start + end_pos];
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    // Fallback: look for the tag name and extract content between > and <
    if let Some(tag_pos) = xml_lower.find(&tag_lower) {
        let after_tag = &xml[tag_pos..];
        if let Some(gt_pos) = after_tag.find('>') {
            let content_start = gt_pos + 1;
            let after_gt = &after_tag[content_start..];
            if let Some(lt_pos) = after_gt.find('<') {
                let content = after_gt[..lt_pos].trim();
                if !content.is_empty() {
                    return Some(content.to_string());
                }
            }
        }
    }

    None
}
