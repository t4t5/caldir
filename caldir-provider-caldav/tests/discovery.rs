//! Endpoint discovery against servers that only advertise a well-known URI.

use caldir_provider_caldav::caldav::ops::discover_endpoints;
use http_body_util::{BodyExt, Full};
use hyper::{
    Response,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, task::JoinHandle};

const PRINCIPAL: &str = "/dav/principals/user/jane/";
const CONTEXT: &str = "/dav/calendars";
const HOME: &str = "/dav/calendars/jane/";

fn multistatus(property: &str, href: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><multistatus xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav"><response><href>/</href><propstat><prop><{property}><href>{href}</href></{property}></prop><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>"#
    )
}

/// A Fastmail-shaped server: nothing at the root, everything under a context path
/// that is only discoverable through `/.well-known/caldav`.
struct Server {
    url: String,
    task: JoinHandle<()>,
}
impl Server {
    async fn new(well_known: bool, reject_context: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let service = service_fn(move |request: hyper::Request<Incoming>| async move {
                    let (parts, body) = request.into_parts();
                    body.collect().await?;
                    let mut response = Response::builder();
                    let (status, payload) = match (parts.method.as_str(), parts.uri.path()) {
                        ("GET", "/.well-known/caldav") if well_known => {
                            response = response.header("Location", CONTEXT);
                            (301, String::new())
                        }
                        (_, path) if reject_context && path.starts_with("/dav/") => {
                            (401, String::new())
                        }
                        ("PROPFIND", CONTEXT) => {
                            (207, multistatus("current-user-principal", PRINCIPAL))
                        }
                        ("PROPFIND", PRINCIPAL) => (207, multistatus("C:calendar-home-set", HOME)),
                        _ => (404, String::new()),
                    };
                    Ok::<_, hyper::Error>(
                        response
                            .status(status)
                            .header("Content-Type", "application/xml; charset=utf-8")
                            .body(Full::new(Bytes::from(payload)))
                            .unwrap(),
                    )
                });
                http1::Builder::new()
                    .keep_alive(false)
                    .serve_connection(TokioIo::new(socket), service)
                    .await
                    .unwrap();
            }
        });
        Self { url, task }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn discovers_endpoints_through_the_well_known_uri() {
    let server = Server::new(true, false).await;
    let endpoints = discover_endpoints(&server.url, "jane", "pw").await.unwrap();
    let base = server.url.trim_end_matches('/');
    assert_eq!(endpoints.principal_url, format!("{base}{PRINCIPAL}"));
    assert_eq!(endpoints.calendar_home_url, format!("{base}{HOME}"));
}

#[tokio::test]
async fn reports_the_original_failure_when_there_is_no_well_known_uri() {
    let server = Server::new(false, false).await;
    let error = match discover_endpoints(&server.url, "jane", "pw").await {
        Ok(_) => panic!("expected discovery to fail"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("Failed to find current user principal"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn reports_the_context_failure_after_well_known_discovery() {
    let server = Server::new(true, true).await;
    let error = match discover_endpoints(&server.url, "jane", "wrong-password").await {
        Ok(_) => panic!("expected discovery to fail"),
        Err(error) => error,
    };
    let error = format!("{error:#}");
    assert!(error.contains("401"), "unexpected error: {error}");
    assert!(!error.contains("404"), "unexpected error: {error}");
}
