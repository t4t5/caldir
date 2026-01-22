//! List events within a time range from an iCloud calendar.
//!
//! Uses CalDAV REPORT with calendar-query to fetch events.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::parse_event;
use caldir_core::remote::protocol::ListEvents;

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    // Fetch events using blocking HTTP
    let events = tokio::task::spawn_blocking({
        let session = session.clone();
        let calendar_url = calendar_url.clone();
        let from = cmd.from.clone();
        let to = cmd.to.clone();
        move || fetch_events_caldav(&session, &calendar_url, &from, &to)
    })
    .await
    .context("Task join error")??;

    Ok(events)
}

/// Fetch events via CalDAV REPORT calendar-query.
fn fetch_events_caldav(
    session: &Session,
    calendar_url: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Event>> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Failed to create HTTP client")?;

    // Format dates for CalDAV (needs UTC format: 20250101T000000Z)
    let from_caldav = format_caldav_datetime(from);
    let to_caldav = format_caldav_datetime(to);

    let report_body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <d:getetag/>
    <c:calendar-data/>
  </d:prop>
  <c:filter>
    <c:comp-filter name="VCALENDAR">
      <c:comp-filter name="VEVENT">
        <c:time-range start="{}" end="{}"/>
      </c:comp-filter>
    </c:comp-filter>
  </c:filter>
</c:calendar-query>"#,
        from_caldav, to_caldav
    );

    let (username, password) = session.credentials();

    let response = client
        .request(
            reqwest::Method::from_bytes(b"REPORT").unwrap(),
            calendar_url,
        )
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/xml; charset=utf-8")
        .header("Depth", "1")
        .body(report_body)
        .send()
        .context("Failed to fetch events")?;

    let status = response.status();

    if !status.is_success() && status.as_u16() != 207 {
        anyhow::bail!(
            "Failed to fetch events (status {})",
            status
        );
    }

    let body = response.text().context("Failed to read response body")?;

    // Parse VCALENDAR data from each response
    let events = parse_events_from_multistatus(&body);

    // Debug: if no events found but we got a response, log it
    if events.is_empty() && !body.is_empty() {
        // Check if there's any calendar data at all
        let has_vcalendar = body.contains("VCALENDAR") || body.contains("vcalendar");
        let has_response = body.to_lowercase().contains("response");

        if !has_vcalendar && has_response {
            // Got a multistatus but no calendar data - might be empty calendar
            // This is fine, just return empty
        } else if has_vcalendar {
            // There's calendar data but we failed to parse it
            eprintln!("[icloud] Warning: Found VCALENDAR data but failed to parse events");
            eprintln!("[icloud] Response preview (first 2000 chars):\n{}", &body[..body.len().min(2000)]);
        }
    }

    Ok(events)
}

/// Parse events from CalDAV multistatus response.
fn parse_events_from_multistatus(xml: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let xml_lower = xml.to_lowercase();

    // Find all response elements (handle various namespace prefixes)
    let response_starts = find_all_response_starts(&xml_lower);

    for start_idx in response_starts {
        // Find the end of this response element
        let response_end = find_response_end(&xml_lower[start_idx..])
            .map(|end| start_idx + end)
            .unwrap_or(xml.len());

        let response = &xml[start_idx..response_end];

        // Extract calendar-data (the VCALENDAR content)
        if let Some(ics_content) = extract_calendar_data(response) {
            // Parse the ICS content to extract the event
            if let Some(event) = parse_event(&ics_content) {
                events.push(event);
            }
        }
    }

    events
}

/// Find all starting positions of response elements.
fn find_all_response_starts(xml_lower: &str) -> Vec<usize> {
    let mut positions = Vec::new();

    // Look for response tags with various prefixes
    let patterns = ["<d:response", "<response", "<d:response", "<ns0:response", "<ns1:response"];

    for pattern in patterns {
        let mut search_start = 0;
        while let Some(pos) = xml_lower[search_start..].find(pattern) {
            let absolute_pos = search_start + pos;
            // Verify it's a tag start
            let after = &xml_lower[absolute_pos + pattern.len()..];
            if after.starts_with('>') || after.starts_with(' ') || after.starts_with('/') {
                positions.push(absolute_pos);
            }
            search_start = absolute_pos + 1;
        }
    }

    positions.sort();
    positions.dedup();
    positions
}

/// Find the end of a response element.
fn find_response_end(xml_lower: &str) -> Option<usize> {
    let patterns = ["</d:response>", "</response>", "</ns0:response>", "</ns1:response>"];

    patterns
        .iter()
        .filter_map(|p| xml_lower.find(p).map(|pos| pos + p.len()))
        .min()
}

/// Extract calendar-data content from a response element.
fn extract_calendar_data(response: &str) -> Option<String> {
    let response_lower = response.to_lowercase();

    // Look for calendar-data element
    let marker = "calendar-data";
    if let Some(marker_pos) = response_lower.find(marker) {
        // Find the > that ends the opening tag
        let after_marker = &response[marker_pos..];
        let gt_pos = after_marker.find('>')?;
        let content_start = gt_pos + 1;

        // Find the closing tag (</calendar-data> with any prefix)
        let after_content = &after_marker[content_start..];
        let after_content_lower = after_content.to_lowercase();

        if let Some(close_pos) = after_content_lower.find("calendar-data>") {
            // Find the < before calendar-data (handles </calendar-data> or </c:calendar-data>)
            let before_close = &after_content[..close_pos];
            if let Some(lt_pos) = before_close.rfind('<') {
                let mut content = &after_content[..lt_pos];

                // Handle CDATA wrapper: <![CDATA[...]]>
                let content_trimmed = content.trim();
                if content_trimmed.starts_with("<![CDATA[") {
                    // Strip CDATA start
                    content = &content_trimmed[9..];
                    // Strip CDATA end
                    if let Some(cdata_end) = content.rfind("]]>") {
                        content = &content[..cdata_end];
                    }
                }

                // Unescape XML entities (in case content is not in CDATA)
                let unescaped = content
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'");

                let trimmed = unescaped.trim();
                if !trimmed.is_empty() && trimmed.contains("BEGIN:VCALENDAR") {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    // Fallback: look for VCALENDAR content directly (some servers inline it)
    if let Some(vcal_start) = response.find("BEGIN:VCALENDAR") {
        if let Some(vcal_end) = response.find("END:VCALENDAR") {
            let content = &response[vcal_start..vcal_end + 13]; // +13 for "END:VCALENDAR"
            return Some(content.to_string());
        }
    }

    None
}

/// Format a datetime string for CalDAV time-range queries.
/// Input: RFC3339 format (e.g., "2025-01-01T00:00:00Z", "2025-01-01T00:00:00+00:00", or "2025-01-01")
/// Output: CalDAV format (e.g., "20250101T000000Z")
fn format_caldav_datetime(datetime: &str) -> String {
    // Remove timezone offset if present (e.g., +00:00 or -05:00)
    let without_offset = if let Some(plus_pos) = datetime.rfind('+') {
        &datetime[..plus_pos]
    } else if let Some(minus_pos) = datetime.rfind('-') {
        // Check if this minus is part of the date (YYYY-MM-DD) or timezone
        // Timezone offset minus comes after 'T'
        if datetime.contains('T') && minus_pos > datetime.find('T').unwrap_or(0) {
            &datetime[..minus_pos]
        } else {
            datetime
        }
    } else {
        datetime
    };

    // Remove hyphens, colons, periods (for fractional seconds), and keep only digits, T, and Z
    let cleaned: String = without_offset
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .collect();

    // Ensure proper format: YYYYMMDDTHHMMSSZ
    if cleaned.len() >= 8 {
        if cleaned.contains('T') {
            // Has time component - take first 15 chars (YYYYMMDDTHHMMSS) and add Z
            let base = if cleaned.len() > 15 {
                &cleaned[..15]
            } else {
                &cleaned
            };
            if base.ends_with('Z') {
                base.to_string()
            } else {
                format!("{}Z", base)
            }
        } else {
            // Date only - add time
            format!("{}T000000Z", &cleaned[..8.min(cleaned.len())])
        }
    } else {
        cleaned
    }
}
