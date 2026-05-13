//! Pure CalDAV operations taking credentials and URLs as parameters.
//!
//! These functions are provider-agnostic and can be used by any CalDAV-based
//! provider (iCloud, generic CalDAV, etc.).

use anyhow::{Context, Result};
use caldir_core::Event;
use http::{Method, Request, Uri};
use libdav::dav::{GetEtag, make_relative_url};
use libdav::requests::{DavRequest, ParseResponseError};

use crate::caldav::{CalDavClient_, event_url, url_to_href};

pub mod create_event;
pub mod delete_event;
pub mod discover;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

pub use create_event::create_event;
pub use delete_event::delete_event;
pub use discover::{DiscoveredEndpoints, discover_endpoints};
pub use list_calendars::{RawCalendar, list_calendars_raw};
pub use list_events::fetch_events;
pub use update_event::update_event;

/// Parse the first valid event out of a single-resource ICS document.
///
/// CalDAV resources contain a single VEVENT (plus any RECURRENCE-ID overrides);
/// we take the first one that parses cleanly and silently drop malformed events.
pub(super) fn parse_event(ics: &str) -> Option<Event> {
    Event::from_ics_str(ics)
        .ok()?
        .into_iter()
        .find_map(Result::ok)
}

/// Result of looking up a CalDAV resource by UID.
pub(super) struct EventLocation {
    pub href: String,
    pub etag: String,
}

/// Find an event's href and etag on the server.
///
/// First tries the conventional `{uid}.ics` URL (works for iCloud, most CalDAV servers).
/// If that returns 404, falls back to a UID-based calendar-query REPORT (for servers
/// like Runbox/Sabre that use server-assigned filenames).
pub(super) async fn find_event_location(
    caldav: &CalDavClient_,
    calendar_href: &str,
    calendar_url: &str,
    uid: &str,
) -> Result<EventLocation> {
    // Fast path: try {uid}.ics directly
    let uid_href = url_to_href(&event_url(calendar_url, uid));
    if let Ok(etag_response) = caldav.request(GetEtag::new(&uid_href)).await {
        return Ok(EventLocation {
            href: uid_href,
            etag: etag_response.etag,
        });
    }

    // Fallback: query by UID (handles servers with non-standard resource filenames)
    caldav
        .request(FindEventByUid::new(calendar_href, uid))
        .await
        .context("Failed to find event on server - it may have been deleted")
}

// ============================================================================
// Custom CalDAV request to find an event by UID
// ============================================================================

/// Request to find a calendar resource by its UID property.
///
/// Uses a CalDAV calendar-query REPORT with a prop-filter on UID.
/// Returns the href and etag of the matching resource.
struct FindEventByUid<'a> {
    collection_href: &'a str,
    uid: &'a str,
}

impl<'a> FindEventByUid<'a> {
    fn new(collection_href: &'a str, uid: &'a str) -> Self {
        Self {
            collection_href,
            uid,
        }
    }
}

impl DavRequest for FindEventByUid<'_> {
    type Response = EventLocation;
    type ParseError = ParseResponseError;
    type Error<E> = libdav::dav::WebDavError<E>;

    fn prepare_request(&self, base_url: Uri) -> std::result::Result<Request<String>, http::Error> {
        let body = format!(
            r#"<C:calendar-query xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
    <prop>
        <getetag/>
    </prop>
    <C:filter>
        <C:comp-filter name="VCALENDAR">
            <C:comp-filter name="VEVENT">
                <C:prop-filter name="UID">
                    <C:text-match collation="i;octet">{}</C:text-match>
                </C:prop-filter>
            </C:comp-filter>
        </C:comp-filter>
    </C:filter>
</C:calendar-query>"#,
            self.uid
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

        let text = std::str::from_utf8(body)?;
        let doc = roxmltree::Document::parse(text)?;
        let root = doc.root_element();

        for response in root
            .descendants()
            .filter(|n| n.tag_name().name() == "response")
        {
            let href = response
                .descendants()
                .find(|n| n.tag_name().name() == "href")
                .and_then(|n| n.text());
            let etag = response
                .descendants()
                .find(|n| n.tag_name().name() == "getetag")
                .and_then(|n| n.text());

            if let (Some(href), Some(etag)) = (href, etag) {
                return Ok(EventLocation {
                    href: href.to_string(),
                    etag: etag.to_string(),
                });
            }
        }

        Err(ParseResponseError::BadStatusCode(
            http::StatusCode::NOT_FOUND,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_extracts_uid_and_summary() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:evt-1@caldir\r\nDTSTART:20260615T100000Z\r\nDTEND:20260615T110000Z\r\nSUMMARY:Test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

        let event = parse_event(ics).expect("should parse");
        assert_eq!(event.uid.as_str(), "evt-1@caldir");
        assert_eq!(event.summary.as_deref(), Some("Test"));
    }

    #[test]
    fn parse_event_returns_none_on_invalid_ics() {
        assert!(parse_event("not ics").is_none());
    }
}
