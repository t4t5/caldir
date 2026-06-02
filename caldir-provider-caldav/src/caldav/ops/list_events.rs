//! Fetch events from a CalDAV calendar within a time range.

use anyhow::{Context, Result};
use caldir_core::Event;
use http::{Method, Request, Uri};
use libdav::dav::make_relative_url;
use libdav::requests::{DavRequest, ParseResponseError};

use crate::caldav::{create_caldav_client, format_caldav_datetime, url_to_href};

use super::parse_event;

/// Fetch events from a CalDAV calendar within a time range.
pub async fn fetch_events(
    username: &str,
    password: &str,
    calendar_url: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Event>> {
    let caldav = create_caldav_client(calendar_url, username, password)?;

    let calendar_href = url_to_href(calendar_url);

    let from_caldav = format_caldav_datetime(from);
    let to_caldav = format_caldav_datetime(to);

    let response = caldav
        .request(GetCalendarResourcesInRange::new(
            &calendar_href,
            &from_caldav,
            &to_caldav,
        ))
        .await
        .context("Failed to fetch calendar resources")?;

    let mut events = Vec::new();
    for resource in response.resources {
        if let Some(event) = parse_event(&resource.data) {
            events.push(event);
        }
    }

    Ok(events)
}

// ============================================================================
// Custom CalDAV request for time-range filtered calendar queries
// ============================================================================

/// Request to fetch calendar resources with server-side time-range filtering.
///
/// This uses the CalDAV calendar-query REPORT with a time-range filter,
/// which is much more efficient than fetching all events and filtering locally.
struct GetCalendarResourcesInRange<'a> {
    collection_href: &'a str,
    start: &'a str,
    end: &'a str,
}

impl<'a> GetCalendarResourcesInRange<'a> {
    /// Create a new request to fetch calendar resources within a time range.
    ///
    /// `start` and `end` must be in CalDAV format: `YYYYMMDDTHHMMSSZ`
    fn new(collection_href: &'a str, start: &'a str, end: &'a str) -> Self {
        Self {
            collection_href,
            start,
            end,
        }
    }
}

/// A fetched calendar resource with its ICS data.
#[derive(Debug)]
struct CalendarResource {
    _href: String,
    _etag: Option<String>,
    data: String,
}

/// Response from a [`GetCalendarResourcesInRange`] request.
#[derive(Debug)]
struct GetCalendarResourcesInRangeResponse {
    resources: Vec<CalendarResource>,
}

impl DavRequest for GetCalendarResourcesInRange<'_> {
    type Response = GetCalendarResourcesInRangeResponse;
    type ParseError = ParseResponseError;
    type Error<E> = libdav::dav::WebDavError<E>;

    fn prepare_request(&self, base_url: Uri) -> std::result::Result<Request<String>, http::Error> {
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

        Request::builder()
            .method(Method::from_bytes(b"REPORT")?)
            .uri(make_relative_url(base_url, self.collection_href)?)
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(body)
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
fn parse_calendar_resources(
    body: &[u8],
) -> std::result::Result<Vec<CalendarResource>, ParseResponseError> {
    let text = std::str::from_utf8(body)?;
    let doc = roxmltree::Document::parse(text)?;
    let root = doc.root_element();

    let mut resources = Vec::new();

    // Find all <response> elements
    for response in root
        .descendants()
        .filter(|n| n.tag_name().name() == "response")
    {
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
            resources.push(CalendarResource {
                _href: href,
                _etag: etag,
                data,
            });
        }
    }

    Ok(resources)
}
