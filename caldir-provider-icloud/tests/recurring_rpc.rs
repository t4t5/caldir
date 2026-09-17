#[path = "../../caldir-provider-caldav/tests/support/mod.rs"]
mod support;

use std::process::Stdio;
use support::{Server, calendar, component, event};
use tokio::{io::AsyncWriteExt, process::Command};

fn storage(server: &Server) -> tempfile::TempDir {
    let storage = tempfile::tempdir().unwrap();
    std::fs::create_dir(storage.path().join("session")).unwrap();
    std::fs::write(storage.path().join("session/fake_example_com.toml"), format!("apple_id = 'fake@example.com'\napp_password = 'fake'\nprincipal_url = {:?}\ncalendar_home_url = {:?}\n", server.url, server.url)).unwrap();
    storage
}

async fn rpc(
    server: &Server,
    storage: &tempfile::TempDir,
    command: &str,
    event: Option<&caldir_core::Event>,
    from: &str,
) -> serde_json::Value {
    let mut params = serde_json::json!({
        "icloud_account": "fake@example.com",
        "icloud_calendar_url": server.url,
        "from": from,
        "to": "2027-01-01T00:00:00Z",
    });
    if let Some(event) = event {
        params["event"] = event.to_ics_string().into();
    }
    let request = serde_json::json!({"command": command, "params": params});
    let mut child = Command::new(env!("CARGO_BIN_EXE_caldir-provider-icloud"))
        .env("CALDIR_PROVIDER_STORAGE_DIR", storage.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    let output = child.wait_with_output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "success", "{response}");
    response["data"].clone()
}

#[tokio::test]
async fn rpc_preserves_recurrence_identity_for_icloud_writes() {
    let server = Server::new(Some(calendar(&[component(None)])), true).await;
    server.state.lock().unwrap().reject_uid_filter = true;
    let storage = storage(&server);
    let rpc = async |command: &str, event: Option<&caldir_core::Event>| {
        rpc(&server, &storage, command, event, "2026-01-01T00:00:00Z").await
    };

    let mut moved = event(Some("20260928T070000Z"));
    moved.start = event(Some("20260929T070000Z"))
        .recurrence_id
        .unwrap()
        .as_event_time()
        .clone();
    moved
        .x_properties
        .push(caldir_core::XProperty::new("X-CUSTOM", "keep"));
    let mut marked = moved.clone();
    marked.x_properties.push(caldir_core::XProperty::new(
        "X-RECURRENCE-EXCEPTION",
        "True",
    ));
    for command in ["create_event", "create_event", "update_event"] {
        let response = rpc(command, Some(&marked)).await;
        let returned = caldir_core::Event::from_ics_str(response.as_str().unwrap())
            .unwrap()
            .remove(0)
            .unwrap();
        assert_eq!(returned, moved);
        assert_eq!(server.events(), vec![event(None), moved.clone()]);
    }
    assert_eq!(rpc("list_events", None).await.as_array().unwrap().len(), 2);
    rpc("delete_event", Some(&moved)).await;
    assert_eq!(server.events().len(), 1);
    assert!(server.events()[0].recurrence_id.is_none());
    rpc("delete_event", Some(&event(None))).await;
    assert!(server.events().is_empty());
    let state = server.state.lock().unwrap();
    let deletion = state
        .requests
        .iter()
        .find(|r| r.method == "DELETE")
        .unwrap();
    assert!(deletion.header("if-match").is_some());
}

#[tokio::test]
async fn query_window_markers_do_not_change_icloud_events() {
    let original = calendar(&[component(None), component(Some("20260928T070000Z"))])
        .replace("END:VEVENT", "X-CUSTOM:keep\r\nEND:VEVENT");
    let server = Server::new(Some(original.clone()), false).await;
    let storage = storage(&server);
    let parse = |data: serde_json::Value| -> Vec<caldir_core::Event> {
        data.as_array()
            .unwrap()
            .iter()
            .map(|ics| {
                caldir_core::Event::from_ics_str(ics.as_str().unwrap())
                    .unwrap()
                    .remove(0)
                    .unwrap()
            })
            .collect()
    };
    let narrow = parse(
        rpc(
            &server,
            &storage,
            "list_events",
            None,
            "2026-01-01T00:00:00Z",
        )
        .await,
    );
    server.state.lock().unwrap().report_data =
        Some(original.replace("END:VEVENT", "X-RECURRENCE-EXCEPTION:True\r\nEND:VEVENT"));
    let wide = parse(
        rpc(
            &server,
            &storage,
            "list_events",
            None,
            "1970-01-01T00:00:00Z",
        )
        .await,
    );
    assert_eq!(wide, narrow);
    assert_eq!(wide, server.events());
    assert!(wide[1].recurrence_id.is_some());
    assert_eq!(
        wide[1].x_properties,
        vec![caldir_core::XProperty::new("X-CUSTOM", "keep")]
    );
    let state = server.state.lock().unwrap();
    assert!(state.requests.iter().all(|r| r.method == "REPORT"));
    assert!(
        state.requests[0]
            .body
            .contains("start=\"20260101T000000Z\"")
    );
    assert!(
        state.requests[1]
            .body
            .contains("start=\"19700101T000000Z\"")
    );
}
