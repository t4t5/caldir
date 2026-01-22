//! CalDAV client helpers for iCloud using libdav.
//!
//! Provides utilities for creating libdav CalDav clients with iCloud authentication.

use anyhow::{Context, Result};
use http::Uri;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use libdav::{CalDavClient, dav::WebDavClient};
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
