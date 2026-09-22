//! CalDAV client helpers using libdav.
//!
//! Provides utilities for creating libdav CalDav clients with basic auth.

use anyhow::{Context, Result};
use http::Uri;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use libdav::CalDavClient;
use libdav::caldav::service_for_url;
use libdav::dav::WebDavClient;
use tower::ServiceBuilder;
use tower_http::{auth::AddAuthorization, follow_redirect::FollowRedirect};

/// Type alias for the authenticated HTTP client, without redirect following.
type AuthClient = AddAuthorization<
    Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        String,
    >,
>;

/// Type alias for the HTTP client with auth and redirect following.
type HttpClient = FollowRedirect<AuthClient>;

/// Type alias for our CalDAV client.
pub type CalDavClient_ = CalDavClient<HttpClient>;

/// Create a libdav CalDavClient configured with basic auth and redirect following.
pub fn create_caldav_client(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<CalDavClient_> {
    let uri = parse_base_url(base_url)?;

    // Add redirect following (some servers redirect to user-specific hosts)
    let client = ServiceBuilder::new()
        .layer(tower_http::follow_redirect::FollowRedirectLayer::new())
        .service(create_auth_client(username, password)?);

    let webdav = WebDavClient::new(uri, client);
    Ok(CalDavClient::new(webdav))
}

/// Resolve a server's CalDAV context path via its well-known URI (RFC 6764).
///
/// Returns `None` when `/.well-known/caldav` does not redirect anywhere.
pub async fn find_well_known_context_url(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<Option<String>> {
    let uri = parse_base_url(base_url)?;
    let service =
        service_for_url(&uri).with_context(|| format!("Invalid base URL: {}", base_url))?;
    let host = uri
        .host()
        .with_context(|| format!("Base URL has no host: {}", base_url))?
        .to_owned();
    let port = uri.port_u16().unwrap_or(service.default_port());

    // Deliberately not the redirect-following client: the redirect target is the answer.
    let webdav = WebDavClient::new(uri, create_auth_client(username, password)?);

    let context_url = webdav
        .find_context_path(service, &host, port)
        .await
        .context("Failed to resolve the well-known CalDAV URL")?;

    Ok(context_url.map(|uri| uri.to_string()))
}

fn parse_base_url(base_url: &str) -> Result<Uri> {
    base_url
        .parse()
        .with_context(|| format!("Invalid base URL: {}", base_url))
}

fn create_auth_client(username: &str, password: &str) -> Result<AuthClient> {
    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .context("Failed to load native TLS roots")?
        .https_or_http()
        .enable_http1()
        .build();

    let http_client = Client::builder(TokioExecutor::new()).build(https_connector);

    Ok(AddAuthorization::basic(http_client, username, password))
}

/// Build an absolute URL from a client's base URL and a relative path.
pub fn absolute_url(client: &CalDavClient_, path: &str) -> String {
    format!(
        "{}://{}{}",
        client.base_url().scheme_str().unwrap_or("https"),
        client
            .base_url()
            .authority()
            .map(|a| a.as_str())
            .unwrap_or("localhost"),
        path
    )
}

/// Build the URL for an event resource.
pub fn event_url(calendar_url: &str, event_uid: &str) -> String {
    let base = calendar_url.trim_end_matches('/');
    format!("{}/{}.ics", base, event_uid)
}

/// Extract the href path from a full URL.
///
/// Converts "https://server.com/123/calendars/abc/" to "/123/calendars/abc/"
pub fn url_to_href(url: &str) -> String {
    if let Ok(uri) = url.parse::<Uri>() {
        uri.path().to_string()
    } else {
        url.to_string()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_caldav_datetime_handles_rfc3339_zulu() {
        assert_eq!(
            format_caldav_datetime("2025-06-15T10:30:00Z"),
            "20250615T103000Z"
        );
    }

    #[test]
    fn format_caldav_datetime_strips_positive_offset() {
        assert_eq!(
            format_caldav_datetime("2025-06-15T10:30:00+02:00"),
            "20250615T103000Z"
        );
    }

    #[test]
    fn format_caldav_datetime_strips_negative_offset() {
        assert_eq!(
            format_caldav_datetime("2025-06-15T10:30:00-05:00"),
            "20250615T103000Z"
        );
    }

    #[test]
    fn format_caldav_datetime_pads_date_only_to_midnight() {
        assert_eq!(format_caldav_datetime("2025-06-15"), "20250615T000000Z");
    }

    #[test]
    fn event_url_joins_calendar_and_uid() {
        assert_eq!(
            event_url("https://server/dav/cal/1/", "abc123"),
            "https://server/dav/cal/1/abc123.ics"
        );
    }

    #[test]
    fn event_url_handles_trailing_slash() {
        // Trailing-slash and no-trailing-slash forms produce the same URL
        assert_eq!(
            event_url("https://server/dav/cal/1", "x"),
            event_url("https://server/dav/cal/1/", "x")
        );
    }

    #[test]
    fn url_to_href_strips_scheme_and_host() {
        assert_eq!(
            url_to_href("https://caldav.fastmail.com/dav/calendars/1/"),
            "/dav/calendars/1/"
        );
    }

    #[test]
    fn url_to_href_passes_through_unparseable() {
        assert_eq!(url_to_href("not a url"), "not a url");
    }
}
