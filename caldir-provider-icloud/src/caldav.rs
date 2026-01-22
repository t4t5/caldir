//! CalDAV client helpers for iCloud using libdav.
//!
//! Provides utilities for creating libdav CalDav clients with iCloud authentication.

use anyhow::{Context, Result};
use http::{Method, Uri};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use libdav::dav::WebDavClient;
use libdav::requests::{DavRequest, ParseResponseError, PreparedRequest};
use libdav::CalDavClient;
use tower::ServiceBuilder;
use tower_http::{auth::AddAuthorization, follow_redirect::FollowRedirect};

/// Type alias for the HTTP client with auth and redirect following.
type HttpClient = FollowRedirect<AddAuthorization<Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, String>>>;

/// Type alias for our CalDAV client.
pub type ICloudCalDavClient = CalDavClient<HttpClient>;

/// Create a libdav CalDavClient configured for iCloud.
///
/// The client is configured with:
/// - Basic authentication using the provided credentials
/// - Automatic redirect following (iCloud redirects to user-specific servers)
/// - HTTPS support
pub fn create_caldav_client(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<ICloudCalDavClient> {
    let uri: Uri = base_url
        .parse()
        .with_context(|| format!("Invalid base URL: {}", base_url))?;

    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .context("Failed to load native TLS roots")?
        .https_or_http()
        .enable_http1()
        .build();

    let http_client = Client::builder(TokioExecutor::new()).build(https_connector);

    // Add basic auth
    let auth_client = AddAuthorization::basic(http_client, username, password);

    // Add redirect following (iCloud redirects to pXX-caldav.icloud.com)
    let client = ServiceBuilder::new()
        .layer(tower_http::follow_redirect::FollowRedirectLayer::new())
        .service(auth_client);

    let webdav = WebDavClient::new(uri, client);
    Ok(CalDavClient::new(webdav))
}

/// Build the URL for an event resource.
pub fn event_url(calendar_url: &str, event_uid: &str) -> String {
    let base = calendar_url.trim_end_matches('/');
    format!("{}/{}.ics", base, event_uid)
}

/// Extract the href path from a full URL.
///
/// Converts "https://pXX-caldav.icloud.com/123/calendars/abc/" to "/123/calendars/abc/"
pub fn url_to_href(url: &str) -> String {
    if let Ok(uri) = url.parse::<Uri>() {
        uri.path().to_string()
    } else {
        url.to_string()
    }
}

// ============================================================================
// Custom CalDAV request for time-range filtered calendar queries
// ============================================================================

/// Request to fetch calendar resources with server-side time-range filtering.
///
/// This uses the CalDAV calendar-query REPORT with a time-range filter,
/// which is much more efficient than fetching all events and filtering locally.
pub struct GetCalendarResourcesInRange<'a> {
    collection_href: &'a str,
    start: &'a str,
    end: &'a str,
}

impl<'a> GetCalendarResourcesInRange<'a> {
    /// Create a new request to fetch calendar resources within a time range.
    ///
    /// `start` and `end` must be in CalDAV format: `YYYYMMDDTHHMMSSZ`
    pub fn new(collection_href: &'a str, start: &'a str, end: &'a str) -> Self {
        Self {
            collection_href,
            start,
            end,
        }
    }
}

/// A fetched calendar resource with its ICS data.
#[derive(Debug)]
pub struct CalendarResource {
    pub href: String,
    pub etag: Option<String>,
    pub data: String,
}

/// Response from a [`GetCalendarResourcesInRange`] request.
#[derive(Debug)]
pub struct GetCalendarResourcesInRangeResponse {
    pub resources: Vec<CalendarResource>,
}

impl DavRequest for GetCalendarResourcesInRange<'_> {
    type Response = GetCalendarResourcesInRangeResponse;
    type ParseError = ParseResponseError;
    type Error<E> = libdav::dav::WebDavError<E>;

    fn prepare_request(&self) -> std::result::Result<PreparedRequest, http::Error> {
        // Build calendar-query REPORT with time-range filter
        let body = format!(
            r#"<C:calendar-query xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
    <prop>
        <getetag/>
        <C:calendar-data/>
    </prop>
    <C:filter>
        <C:comp-filter name="VCALENDAR">
            <C:comp-filter name="VEVENT">
                <C:time-range start="{}" end="{}"/>
            </C:comp-filter>
        </C:comp-filter>
    </C:filter>
</C:calendar-query>"#,
            self.start, self.end
        );

        Ok(PreparedRequest {
            method: Method::from_bytes(b"REPORT")?,
            path: self.collection_href.to_string(),
            body,
            headers: vec![("Depth".to_string(), "1".to_string())],
        })
    }

    fn parse_response(
        &self,
        parts: &http::response::Parts,
        body: &[u8],
    ) -> std::result::Result<Self::Response, ParseResponseError> {
        if !parts.status.is_success() {
            return Err(ParseResponseError::BadStatusCode(parts.status));
        }

        let resources = parse_calendar_resources(body)?;
        Ok(GetCalendarResourcesInRangeResponse { resources })
    }
}

/// Parse calendar resources from a CalDAV multistatus response.
fn parse_calendar_resources(body: &[u8]) -> std::result::Result<Vec<CalendarResource>, ParseResponseError> {
    let text = std::str::from_utf8(body)?;
    let doc = roxmltree::Document::parse(text)?;
    let root = doc.root_element();

    let mut resources = Vec::new();

    // Find all <response> elements
    for response in root.descendants().filter(|n| n.tag_name().name() == "response") {
        // Get href
        let href = response
            .descendants()
            .find(|n| n.tag_name().name() == "href")
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        let Some(href) = href else { continue };

        // Get etag
        let etag = response
            .descendants()
            .find(|n| n.tag_name().name() == "getetag")
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        // Get calendar-data
        let data = response
            .descendants()
            .find(|n| n.tag_name().name() == "calendar-data")
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        // Only include resources that have calendar data
        if let Some(data) = data {
            resources.push(CalendarResource { href, etag, data });
        }
    }

    Ok(resources)
}

/// Format a date string for CalDAV time-range queries.
///
/// Input: RFC3339 format (e.g., "2025-01-01T00:00:00Z", "2025-01-01T00:00:00+00:00", or "2025-01-01")
/// Output: CalDAV format (e.g., "20250101T000000Z")
pub fn format_caldav_datetime(datetime: &str) -> String {
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
