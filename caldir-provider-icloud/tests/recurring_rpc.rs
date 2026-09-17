#[path = "../../caldir-provider-caldav/tests/support/mod.rs"]
mod support;

use std::process::Stdio;
use support::{Server, calendar, component, event};
use tokio::{io::AsyncWriteExt, process::Command};

#[tokio::test]
async fn rpc_preserves_recurrence_identity_for_icloud_writes() {
    let server = Server::new(Some(calendar(&[component(None)])), true).await;
    server.state.lock().unwrap().reject_uid_filter = true;
    let storage = tempfile::tempdir().unwrap();
    std::fs::create_dir(storage.path().join("session")).unwrap();
    std::fs::write(storage.path().join("session/fake_example_com.toml"), format!("apple_id = 'fake@example.com'\napp_password = 'fake'\nprincipal_url = {:?}\ncalendar_home_url = {:?}\n", server.url, server.url)).unwrap();

    let rpc = async |command: &str, event: Option<&caldir_core::Event>| {
        let mut params = serde_json::json!({
            "icloud_account": "fake@example.com",
            "icloud_calendar_url": server.url,
            "from": "2026-01-01T00:00:00Z",
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
    };

    let mut moved = event(Some("20260928T070000Z"));
    moved.start = event(Some("20260929T070000Z"))
        .recurrence_id
        .unwrap()
        .as_event_time()
        .clone();
    for command in ["create_event", "create_event", "update_event"] {
        let response = rpc(command, Some(&moved)).await;
        let returned = caldir_core::Event::from_ics_str(response.as_str().unwrap())
            .unwrap()
            .remove(0)
            .unwrap();
        assert_eq!(returned.event_instance_id(), moved.event_instance_id());
        assert_eq!(returned.start, moved.start);
        assert_eq!(server.events().len(), 2);
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
