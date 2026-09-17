mod support;

use caldir_provider_caldav::caldav::ops::{create_event, delete_event, fetch_events, update_event};
use support::{Server, calendar, component, event};

const RID: &str = "20260928T070000Z";
const SECOND: &str = "20261005T070000Z";

async fn list(server: &Server) -> anyhow::Result<Vec<caldir_core::Event>> {
    fetch_events("fake", "fake", &server.url, "2026-01-01", "2027-01-01").await
}

#[tokio::test]
async fn lists_all_components_in_any_order() {
    for order in [
        [None, Some(RID), Some(SECOND)],
        [Some(SECOND), None, Some(RID)],
    ] {
        let server = Server::new(Some(calendar(&order.map(component))), false).await;
        let events = list(&server).await.unwrap();
        assert_eq!(events.len(), 3);
        for (actual, rid) in events.iter().zip(order) {
            assert_eq!(actual.event_instance_id(), event(rid).event_instance_id());
        }
    }
}

#[tokio::test]
async fn creates_and_updates_components_at_server_assigned_href() {
    let master = component(None);
    let sibling = component(Some(SECOND));
    let server = Server::new(Some(calendar(&[master.clone(), sibling.clone()])), true).await;
    let mut moved = event(Some(RID));
    moved.start = event(Some(SECOND))
        .recurrence_id
        .unwrap()
        .as_event_time()
        .clone();
    for _ in 0..2 {
        let returned = create_event("fake", "fake", &server.url, moved.clone())
            .await
            .unwrap();
        assert_eq!(returned.event_instance_id(), moved.event_instance_id());
        assert_eq!(returned.start, moved.start);
        assert_eq!(server.events().len(), 3);
    }
    moved.summary = Some("Changed override".into());
    let returned = update_event("fake", "fake", &server.url, moved.clone())
        .await
        .unwrap();
    assert_eq!(returned.summary, moved.summary);
    assert_eq!(returned.event_instance_id(), moved.event_instance_id());
    {
        let state = server.state.lock().unwrap();
        assert!(state.data.as_ref().unwrap().contains(&master));
        assert!(state.data.as_ref().unwrap().contains(&sibling));
        for request in state.requests.iter().filter(|r| r.method == "PUT") {
            assert_eq!(request.path, "/calendar/server-assigned.ics");
            assert!(request.header("if-match").is_some());
            assert_eq!(
                request
                    .headers
                    .iter()
                    .filter(|(n, _)| n == "content-type")
                    .count(),
                1
            );
        }
    }
    let mut master = event(None);
    master.summary = Some("Changed master".into());
    let returned = update_event("fake", "fake", &server.url, master.clone())
        .await
        .unwrap();
    assert_eq!(returned.event_instance_id(), master.event_instance_id());
    assert_eq!(server.events().len(), 3);
    assert!(server.events().contains(&moved));
}

#[tokio::test]
async fn creates_and_deletes_series_in_either_order() {
    for order in [[None, Some(RID)], [Some(RID), None]] {
        let server = Server::new(None, false).await;
        for rid in order {
            create_event("f", "f", &server.url, event(rid))
                .await
                .unwrap();
        }
        assert_eq!(server.events().len(), 2);
        for (i, rid) in order.into_iter().enumerate() {
            delete_event("f", "f", &server.url, &event(rid).event_instance_id())
                .await
                .unwrap();
            assert_eq!(server.events().len(), 1 - i);
        }
        delete_event("f", "f", &server.url, &event(None).event_instance_id())
            .await
            .unwrap();
        let state = server.state.lock().unwrap();
        let writes: Vec<_> = state
            .requests
            .iter()
            .filter(|r| r.method == "PUT" || r.method == "DELETE")
            .collect();
        assert_eq!(writes.len(), 4);
        assert_eq!(writes[0].header("if-none-match"), Some("*"));
        assert_eq!(writes[3].method, "DELETE");
        for request in &writes[1..] {
            assert!(request.header("if-match").is_some());
        }
    }
}

#[tokio::test]
async fn cancellation_exdate_and_override_delete_work_in_either_order() {
    for delete_first in [false, true] {
        let server = Server::new(
            Some(calendar(&[
                component(Some(RID)),
                component(None),
                component(Some(SECOND)),
            ])),
            false,
        )
        .await;
        let master_ics = calendar(&[component(None).replace(
            "RRULE:FREQ=WEEKLY",
            "RRULE:FREQ=WEEKLY\r\nEXDATE:20260928T070000Z",
        )]);
        let master = caldir_core::Event::from_ics_str(&master_ics)
            .unwrap()
            .remove(0)
            .unwrap();
        for delete in [delete_first, !delete_first] {
            if delete {
                delete_event("f", "f", &server.url, &event(Some(RID)).event_instance_id())
                    .await
                    .unwrap();
            } else {
                update_event("f", "f", &server.url, master.clone())
                    .await
                    .unwrap();
            }
        }
        assert_eq!(server.events().len(), 2);
        let state = server.state.lock().unwrap();
        assert!(
            state
                .data
                .as_ref()
                .unwrap()
                .contains("EXDATE:20260928T070000Z")
        );
        assert!(
            state
                .data
                .as_ref()
                .unwrap()
                .contains(&component(Some(SECOND)))
        );
    }
}

#[tokio::test]
async fn absence_is_distinct_from_read_failures() {
    let server = Server::new(None, false).await;
    assert!(
        update_event("f", "f", &server.url, event(None))
            .await
            .is_err()
    );
    for status in [401, 403, 500] {
        server.state.lock().unwrap().read_status = Some(status);
        assert!(
            create_event("f", "f", &server.url, event(None))
                .await
                .is_err()
        );
        assert!(
            update_event("f", "f", &server.url, event(None))
                .await
                .is_err()
        );
        assert!(
            delete_event("f", "f", &server.url, &event(None).event_instance_id())
                .await
                .is_err()
        );
    }
    assert!(
        !server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.method == "PUT" || r.method == "DELETE")
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        delete_event("f", "f", &url, &event(None).event_instance_id())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn malformed_resources_and_missing_etags_prevent_writes() {
    for data in [
        "not ics".into(),
        calendar(&[
            component(None),
            "BEGIN:VEVENT\r\nUID:series\r\nEND:VEVENT\r\n".into(),
        ]),
    ] {
        let server = Server::new(Some(data), false).await;
        assert!(list(&server).await.is_err());
        assert!(
            create_event("f", "f", &server.url, event(Some(RID)))
                .await
                .is_err()
        );
        let error = update_event("f", "f", &server.url, event(None))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("/calendar/series.ics"));
        let error = delete_event("f", "f", &server.url, &event(None).event_instance_id())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("/calendar/series.ics"));
        assert!(
            server
                .state
                .lock()
                .unwrap()
                .requests
                .iter()
                .all(|r| r.method != "PUT" && r.method != "DELETE")
        );
    }
    let server = Server::new(Some(calendar(&[component(None)])), false).await;
    server.state.lock().unwrap().omit_etag = true;
    assert!(
        update_event("f", "f", &server.url, event(None))
            .await
            .is_err()
    );
    assert!(
        delete_event("f", "f", &server.url, &event(None).event_instance_id())
            .await
            .is_err()
    );
    delete_event("f", "f", &server.url, &event(Some(RID)).event_instance_id())
        .await
        .unwrap();
}

#[tokio::test]
async fn concurrent_writes_and_creation_races_preserve_the_winners_data() {
    for initial in [None, Some(calendar(&[component(None)]))] {
        let server = Server::new(initial, false).await;
        let concurrent = calendar(&[component(None), component(Some(SECOND))]);
        server.state.lock().unwrap().concurrent_data = Some(concurrent.clone());
        let error = create_event("f", "f", &server.url, event(Some(RID)))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("412"));
        assert_eq!(
            server.state.lock().unwrap().data.as_deref(),
            Some(concurrent.as_str())
        );
        create_event("f", "f", &server.url, event(Some(RID)))
            .await
            .unwrap();
        assert_eq!(server.events().len(), 3);
    }
    let server = Server::new(Some(calendar(&[component(None)])), false).await;
    server.state.lock().unwrap().write_status = Some(412);
    assert!(
        delete_event("f", "f", &server.url, &event(None).event_instance_id())
            .await
            .unwrap_err()
            .to_string()
            .contains("412")
    );
    assert_eq!(server.events().len(), 1);
}

#[tokio::test]
async fn standalone_crud_and_unavailable_readback_keep_identity() {
    let server = Server::new(None, false).await;
    server.state.lock().unwrap().fail_readback = true;
    let mut standalone = event(None);
    standalone.recurrence = None;
    let returned = create_event("f", "f", &server.url, standalone.clone())
        .await
        .unwrap();
    assert_eq!(returned, standalone);
    server.state.lock().unwrap().fail_readback = false;
    standalone.summary = Some("Edited".into());
    assert_eq!(
        update_event("f", "f", &server.url, standalone.clone())
            .await
            .unwrap(),
        standalone
    );
    delete_event("f", "f", &server.url, &standalone.event_instance_id())
        .await
        .unwrap();
    assert!(server.events().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn core_push_pull_converges_for_moves_and_cancellations() {
    use caldir_core::{Caldir, CaldirConfig, CalendarConfig, DateRange};
    use std::os::unix::fs::PermissionsExt;

    let server = Server::new(
        Some(calendar(&[component(None), component(Some(SECOND))])),
        true,
    )
    .await;
    server.state.lock().unwrap().bump_sibling_timestamps = true;
    let tmp = tempfile::tempdir().unwrap();
    let storage = tmp.path().join("storage");
    std::fs::create_dir_all(storage.join("session")).unwrap();
    std::fs::write(storage.join("session/fake.toml"), format!("server_url = {:?}\nusername = 'fake'\npassword = 'fake'\nprincipal_url = {:?}\ncalendar_home_url = {:?}\n", server.url, server.url, server.url)).unwrap();
    let bin_dir = tmp.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let wrapper = bin_dir.join("caldir-provider-caldav");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexport CALDIR_PROVIDER_STORAGE_DIR={}\nexec {}\n",
            quote(storage.to_str().unwrap()),
            quote(env!("CARGO_BIN_EXE_caldir-provider-caldav"))
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config_path = tmp.path().join("config.toml");
    CaldirConfig::new(tmp.path().join("calendars"), Default::default(), None, None)
        .write(&config_path)
        .unwrap();
    let caldir = Caldir::load_from(config_path)
        .unwrap()
        .with_bundled_providers(bin_dir);
    let config: CalendarConfig = toml::from_str(&format!("[remote]\nprovider = 'caldav'\ncaldav_account = 'fake@127.0.0.1'\ncaldav_calendar_url = {:?}\n", server.url)).unwrap();
    caldir.create_calendar("test", Some(config)).unwrap();
    let mut connection = caldir.connections().remove(0).unwrap();
    let range = DateRange::default();
    let diff = connection.diff(&range).await.unwrap();
    connection.apply_incoming_diff(&diff).unwrap();
    assert!(connection.diff(&range).await.unwrap().is_empty());

    let id = event(Some(RID)).event_instance_id();
    let new_start = event(Some(SECOND))
        .recurrence_id
        .unwrap()
        .as_event_time()
        .clone();
    connection
        .local()
        .update_recurring_instance(&id, |e| {
            e.start = new_start.clone();
        })
        .unwrap();
    let diff = connection.diff(&range).await.unwrap();
    assert_eq!(diff.outgoing().len(), 1);
    connection.apply_outgoing_diff(&diff).await.unwrap();
    let diff = connection.diff(&range).await.unwrap();
    assert!(diff.outgoing().is_empty());
    connection.apply_incoming_diff(&diff).unwrap();
    assert!(connection.diff(&range).await.unwrap().is_empty());
    let moved = connection
        .local()
        .event_by_instance_id(&id)
        .unwrap()
        .unwrap();
    assert_eq!(moved.event().event_instance_id(), id);
    assert_eq!(moved.event().start, new_start);
    assert_eq!(connection.local().events().unwrap().len(), 3);

    connection.local().delete_recurring_instance(&id).unwrap();
    let diff = connection.diff(&range).await.unwrap();
    assert_eq!(diff.outgoing().len(), 2);
    connection.apply_outgoing_diff(&diff).await.unwrap();
    let diff = connection.diff(&range).await.unwrap();
    assert!(diff.outgoing().is_empty());
    connection.apply_incoming_diff(&diff).unwrap();
    assert!(connection.diff(&range).await.unwrap().is_empty());
    assert_eq!(connection.local().events().unwrap().len(), 2);
    assert!(
        connection
            .local()
            .event_by_instance_id(&id)
            .unwrap()
            .is_none()
    );
    assert_eq!(server.events().len(), 2);
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .contains("EXDATE:20260928T070000Z")
    );
}

#[tokio::test]
async fn readback_selects_master_when_an_override_comes_first() {
    let server = Server::new(
        Some(calendar(&[component(Some(RID)), component(None)])),
        false,
    )
    .await;
    let mut master = event(None);
    master.summary = Some("Changed master".into());
    let returned = update_event("f", "f", &server.url, master.clone())
        .await
        .unwrap();
    assert_eq!(returned, master);
    server.state.lock().unwrap().fail_readback = true;
    // A failing GET before mutation must still fail; only post-write readback is optional.
    assert!(
        update_event("f", "f", &server.url, event(Some(RID)))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn override_readback_failure_preserves_the_callers_identity() {
    let server = Server::new(Some(calendar(&[component(None)])), false).await;
    server.state.lock().unwrap().fail_readback = true;
    let moved = event(Some(RID));
    let returned = create_event("f", "f", &server.url, moved.clone())
        .await
        .unwrap();
    assert_eq!(returned, moved);
    assert_eq!(server.events().len(), 2);
}

#[tokio::test]
async fn rejected_component_deletes_are_errors_and_preserve_siblings() {
    for status in [403, 412] {
        let original = calendar(&[component(None), component(Some(RID))]);
        let server = Server::new(Some(original.clone()), false).await;
        server.state.lock().unwrap().write_status = Some(status);
        assert!(
            delete_event("f", "f", &server.url, &event(Some(RID)).event_instance_id())
                .await
                .is_err()
        );
        let state = server.state.lock().unwrap();
        assert_eq!(state.data, Some(original));
        let put = state.requests.iter().find(|r| r.method == "PUT").unwrap();
        assert_eq!(put.header("if-match"), Some("\"1\""));
        assert!(!state.requests.iter().any(|r| r.method == "DELETE"));
    }
}
