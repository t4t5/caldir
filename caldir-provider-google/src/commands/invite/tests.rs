use super::*;
use crate::google_event::FromGoogle;
use caldir_core::{ParticipationStatus, Reminder, XProperty};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

fn google_invite(reminders: Value) -> Value {
    json!({
        "id": "invite",
        "iCalUID": "invite@example.com",
        "summary": "Friday retro",
        "description": "Organizer's description",
        "location": "Organizer's room",
        "start": {"dateTime": "2026-09-18T14:00:00Z"},
        "end": {"dateTime": "2026-09-18T15:00:00Z"},
        "organizer": {"email": "organizer@example.com"},
        "attendees": [
            {"email": "organizer@example.com", "responseStatus": "accepted"},
            {"email": "me@example.com", "responseStatus": "needsAction", "self": true},
            {"email": "other@example.com", "responseStatus": "tentative"},
        ],
        "reminders": reminders,
    })
}

fn local_invite(remote: &Value) -> Event {
    let mut event = Event::from_google(serde_json::from_value(remote.clone()).unwrap()).unwrap();
    event
        .attendees
        .iter_mut()
        .find(|a| a.email == "me@example.com")
        .unwrap()
        .status = Some(ParticipationStatus::Accepted);
    event
}

fn serialized_patch(event: &Event, remote: &Value) -> Value {
    let remote: google_calendar::types::Event = serde_json::from_value(remote.clone()).unwrap();
    let body = invite_patch_body(event, "me@example.com", remote.reminders.as_ref()).unwrap();
    serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap()
}

fn overrides(values: &[(i64, &str)]) -> Value {
    json!({
        "useDefault": false,
        "overrides": values.iter().map(|(minutes, method)| {
            json!({"minutes": minutes, "method": method})
        }).collect::<Vec<_>>()
    })
}

#[test]
fn rsvp_only_omits_email_at_start_and_duplicate_reminders() {
    for reminders in [
        overrides(&[(30, "email")]),
        overrides(&[(0, "popup")]),
        overrides(&[(30, "popup"), (30, "email")]),
        overrides(&[(30, "email"), (0, "email"), (10, "popup")]),
        json!({"useDefault": true}),
        overrides(&[]),
        Value::Null,
    ] {
        let remote = google_invite(reminders);
        let event = local_invite(&remote);
        let body = serialized_patch(&event, &remote);
        assert_eq!(
            body,
            json!({
                "attendeesOmitted": true,
                "attendees": [{"email": "me@example.com", "responseStatus": "accepted"}],
            })
        );
    }
}

#[test]
fn adding_popup_preserves_methods_duplicates_and_literal_zero() {
    let remote = google_invite(overrides(&[(30, "popup"), (0, "email"), (30, "email")]));
    let mut event = local_invite(&remote);
    event.reminders.push(Reminder::from_minutes(60));
    let body = serialized_patch(&event, &remote);
    assert_eq!(
        body["reminders"],
        overrides(&[(0, "email"), (30, "email"), (30, "popup"), (60, "popup"),])
    );
    assert!(
        serde_json::to_string(&body)
            .unwrap()
            .contains("\"minutes\":0")
    );

    event.reminders.reverse();
    let reordered_remote = google_invite(overrides(&[(30, "email"), (30, "popup"), (0, "email")]));
    assert_eq!(serialized_patch(&event, &reordered_remote), body);
}

#[test]
fn adding_duplicate_offset_consumes_remote_match_once() {
    let remote = google_invite(overrides(&[(30, "email")]));
    let mut event = local_invite(&remote);
    event.reminders.push(Reminder::from_minutes(30));
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[(30, "email"), (30, "popup"),])
    );
}

#[test]
fn removing_one_or_all_valarms_never_restores_remote_overrides() {
    let remote = google_invite(overrides(&[(30, "email"), (30, "popup"), (0, "popup")]));
    let mut event = local_invite(&remote);
    event.reminders = vec![Reminder::from_minutes(30), Reminder::from_minutes(0)];
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[(0, "popup"), (30, "email"),])
    );
    event.reminders.clear();
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[])
    );
}

#[test]
fn changing_offset_creates_popup() {
    let remote = google_invite(overrides(&[(30, "email")]));
    let mut event = local_invite(&remote);
    event.reminders = vec![Reminder::from_minutes(15)];
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[(15, "popup")])
    );
}

#[test]
fn defaults_clearing_and_explicit_valarm_precedence() {
    let remote = google_invite(overrides(&[(30, "email")]));
    let mut event = local_invite(&remote);
    event.reminders.clear();
    event
        .x_properties
        .push(XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "true"));
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        json!({
            "useDefault": true, "overrides": [],
        })
    );

    let remote = google_invite(json!({"useDefault": true}));
    for marker in ["FALSE", ""] {
        let mut event = local_invite(&remote);
        event
            .x_properties
            .retain(|p| p.name != GOOGLE_DEFAULT_REMINDERS_PROPERTY);
        if !marker.is_empty() {
            event
                .x_properties
                .push(XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, marker));
        }
        assert_eq!(
            serialized_patch(&event, &remote)["reminders"],
            overrides(&[])
        );
    }
    let mut event = local_invite(&remote);
    event.reminders.push(Reminder::from_minutes(0));
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[(0, "popup")])
    );
}

#[test]
fn maximum_offset_and_five_overrides_are_supported() {
    let remote = google_invite(overrides(&[]));
    let mut event = local_invite(&remote);
    event.reminders = [0, 5, 10, 15, 40320].map(Reminder::from_minutes).to_vec();
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[
            (0, "popup"),
            (5, "popup"),
            (10, "popup"),
            (15, "popup"),
            (40320, "popup"),
        ])
    );
}

struct TestServer {
    url: Url,
    requests: mpsc::UnboundedReceiver<(String, Value)>,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn new(event_id: &str, get_status: u16, get_body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut url = event_url("me@example.com", event_id);
        url.set_scheme("http").unwrap();
        url.set_host(Some("127.0.0.1")).unwrap();
        url.set_port(Some(address.port())).unwrap();
        let (tx, requests) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                let mut request_line = String::new();
                stream.read_line(&mut request_line).await.unwrap();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    stream.read_line(&mut line).await.unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                stream.read_exact(&mut body).await.unwrap();
                let body = if body.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&body).unwrap()
                };
                let is_get = request_line.starts_with("GET ");
                tx.send((request_line.trim().to_owned(), body)).unwrap();
                let (status, response) = if is_get {
                    (get_status, get_body.as_str())
                } else {
                    (200, "{}")
                };
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }

    fn take_requests(&mut self) -> Vec<(String, Value)> {
        let mut requests = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            requests.push(request);
        }
        requests
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn get_and_patch_use_exact_standalone_or_recurring_instance_id() {
    for event_id in ["invite", "series_20260918T140000Z"] {
        let mut remote = google_invite(overrides(&[(30, "email"), (0, "popup")]));
        remote["id"] = json!(event_id);
        if event_id.starts_with("series_") {
            remote["recurringEventId"] = json!("series");
            remote["originalStartTime"] = json!({"dateTime": "2026-09-18T14:00:00Z"});
        }
        let mut event = local_invite(&remote);
        event.reminders.push(Reminder::from_minutes(60));
        let mut server = TestServer::new(event_id, 200, remote.to_string()).await;
        patch_personal_fields_at_url("test-token", server.url.clone(), &event, "me@example.com")
            .await
            .unwrap();
        let requests = server.take_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].0,
            format!("GET /calendar/v3/calendars/me@example.com/events/{event_id} HTTP/1.1")
        );
        assert_eq!(
            requests[1].0,
            format!("PATCH /calendar/v3/calendars/me@example.com/events/{event_id} HTTP/1.1")
        );
        assert_eq!(
            requests[1].1,
            json!({
                "attendeesOmitted": true,
                "attendees": [{"email": "me@example.com", "responseStatus": "accepted"}],
                "reminders": overrides(&[(0, "popup"), (30, "email"), (60, "popup")]),
            })
        );
    }
}

#[tokio::test]
async fn failed_lookup_or_decode_prevents_patch() {
    let remote = google_invite(overrides(&[(30, "email")]));
    let event = local_invite(&remote);
    for (status, body) in [
        (404, "not found"),
        (500, "server error"),
        (200, "invalid JSON"),
    ] {
        let mut server = TestServer::new("series_20260918T140000Z", status, body.into()).await;
        let error = patch_personal_fields_at_url(
            "test-token",
            server.url.clone(),
            &event,
            "me@example.com",
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("before updating personal fields")
        );
        let requests = server.take_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.starts_with("GET "));
        assert!(requests[0].0.contains("/events/series_20260918T140000Z "));
    }
}

#[tokio::test]
async fn unsupported_reminders_prevent_patch() {
    let remote = google_invite(overrides(&[]));
    for (offsets, expected_error) in [
        (vec![-1], "offset -1"),
        (vec![40321], "offset 40321"),
        (vec![i64::MAX], "0 to 40320"),
        (vec![0; 6], "at most 5"),
    ] {
        let mut event = local_invite(&remote);
        event.reminders = offsets.into_iter().map(Reminder::from_minutes).collect();
        let mut server = TestServer::new("invite", 200, remote.to_string()).await;
        let error = patch_personal_fields_at_url(
            "test-token",
            server.url.clone(),
            &event,
            "me@example.com",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains(expected_error), "{error}");
        let requests = server.take_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.starts_with("GET "));
    }
}

#[test]
fn removing_valarms_from_ics_clears_remote_overrides() {
    let remote = google_invite(overrides(&[(30, "email"), (60, "popup")]));
    let mut ics = local_invite(&remote).to_ics_string();
    while let Some(start) = ics.find("BEGIN:VALARM") {
        let end = start + ics[start..].find("END:VALARM\r\n").unwrap() + "END:VALARM\r\n".len();
        ics.replace_range(start..end, "");
    }
    let event = Event::from_ics_str(&ics).unwrap().pop().unwrap().unwrap();
    assert_eq!(
        serialized_patch(&event, &remote)["reminders"],
        overrides(&[])
    );
}
