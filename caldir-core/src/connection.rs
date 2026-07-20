mod error;

use std::collections::HashMap;

use crate::calendar::{CalendarError, SyncBases};
use crate::diff::EventChange;
use crate::event::EventInstanceId;
use crate::{Calendar, CalendarDiff, CalendarEvent, DateRange, Event, Remote, RemoteEvent};
use error::ConnectionError;

/// A connection is a [local calendar] + [remote calendar] pair
pub struct Connection {
    local: Calendar,
    remote: Remote,
}

impl Connection {
    pub fn new(local: Calendar, remote: Remote) -> Self {
        Self { local, remote }
    }

    pub fn local(&self) -> &Calendar {
        &self.local
    }

    pub fn remote(&self) -> &Remote {
        &self.remote
    }

    pub fn read_only(&self) -> bool {
        self.local
            .config()
            .and_then(|c| c.read_only())
            .unwrap_or(false)
    }

    pub async fn diff(&mut self, range: &DateRange) -> Result<CalendarDiff, ConnectionError> {
        let local_events = self.local().events()?;
        let remote_events = self.remote().list_events(range).await?;

        // State migration: in-sync pairs never produce a change to apply, so
        // this is the only place their base can be recorded. Without it,
        // legacy known-id entries would sit on the mtime fallback forever.
        let backfill = bases_to_backfill(
            &local_events,
            &remote_events,
            self.local.state().sync_bases(),
        );
        if !backfill.is_empty() {
            self.local.record_sync_bases(backfill)?;
        }

        let sync_bases = self.local().state().sync_bases();

        let mut diff = CalendarDiff::compute(local_events, remote_events, sync_bases, range);

        if self.read_only() {
            diff.discard_outgoing();
        }

        Ok(diff)
    }

    // pull
    pub fn apply_incoming_diff(&mut self, diff: &CalendarDiff) -> Result<(), ConnectionError> {
        let mut events_by_instance_id: HashMap<EventInstanceId, CalendarEvent> = self
            .local
            .events()?
            .into_iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        let mut sync_bases = Vec::new();

        // Same partial-failure flush pattern as `apply_outgoing_diff`: a
        // local-fs error mid-loop must not drop changes already applied to disk.
        let loop_result = pull_incoming_changes(
            &self.local,
            diff,
            &mut events_by_instance_id,
            &mut sync_bases,
        );

        let record_result = self.local.record_sync_bases(sync_bases);

        loop_result?;
        record_result?;
        Ok(())
    }

    // push
    pub async fn apply_outgoing_diff(
        &mut self,
        diff: &CalendarDiff,
    ) -> Result<(), ConnectionError> {
        let mut events_by_instance_id: HashMap<EventInstanceId, CalendarEvent> = self
            .local
            .events()?
            .into_iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        let mut sync_bases = Vec::new();

        // Handles mid-loop errors gracefully
        let loop_result = push_outgoing_changes(
            &self.remote,
            diff,
            &mut events_by_instance_id,
            &mut sync_bases,
        )
        .await;

        let record_result = self.local.record_sync_bases(sync_bases);

        loop_result?;
        record_result?;
        Ok(())
    }

    // discard
    pub fn discard_outgoing_diff(&self, diff: &CalendarDiff) -> Result<(), ConnectionError> {
        let mut events_by_instance_id: HashMap<EventInstanceId, CalendarEvent> = self
            .local
            .events()?
            .into_iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        for change in diff.outgoing() {
            match change {
                EventChange::Create(event) => {
                    if let Some(cal_event) =
                        events_by_instance_id.remove(&event.event_instance_id())
                    {
                        cal_event.delete().map_err(CalendarError::from)?;
                    }
                }
                EventChange::Update { from, to } => {
                    if let Some(cal_event) = events_by_instance_id.get_mut(&to.event_instance_id())
                    {
                        cal_event
                            .update(from.clone())
                            .map_err(CalendarError::from)?;
                    }
                }
                EventChange::Delete(event) => {
                    let cal_event = self.local.create_event(event.clone())?;
                    events_by_instance_id.insert(cal_event.event().event_instance_id(), cal_event);
                }
            }
        }

        Ok(())
    }
}

/// Events present and identical on both sides whose base is missing (legacy
/// known-id entry) or stale. Both sides agreeing *is* the base — record it.
/// Pairs with no sync state at all are left alone: they were never synced,
/// and recording a base would silently change their delete semantics.
fn bases_to_backfill(
    local_events: &[CalendarEvent],
    remote_events: &[RemoteEvent],
    sync_bases: &SyncBases,
) -> Vec<Event> {
    let local_by_id: HashMap<_, _> = local_events
        .iter()
        .map(|e| (e.event().event_instance_id(), e.event()))
        .collect();

    let mut backfill = Vec::new();

    for remote in remote_events {
        let remote = remote.event();
        let id = remote.event_instance_id();

        let in_sync = local_by_id.get(&id).is_some_and(|local| *local == remote);

        let base_is_current = match sync_bases.get(&id) {
            None => continue,
            Some(Some(base)) => base.as_ref() == remote,
            Some(None) => false,
        };

        if in_sync && !base_is_current {
            backfill.push(remote.clone());
        }
    }

    backfill
}

fn pull_incoming_changes(
    local: &Calendar,
    diff: &CalendarDiff,
    events_by_instance_id: &mut HashMap<EventInstanceId, CalendarEvent>,
    sync_bases: &mut Vec<Event>,
) -> Result<(), ConnectionError> {
    for change in diff.incoming() {
        match change {
            EventChange::Create(event) => {
                let cal_event = local.create_event(event.clone())?;
                let id = cal_event.event().event_instance_id();
                events_by_instance_id.insert(id, cal_event);
                sync_bases.push(event.clone());
            }
            EventChange::Update { to, .. } => {
                if let Some(cal_event) = events_by_instance_id.get_mut(&to.event_instance_id()) {
                    cal_event.update(to.clone()).map_err(CalendarError::from)?;
                }
                sync_bases.push(to.clone());
            }
            EventChange::Delete(event) => {
                if let Some(cal_event) = events_by_instance_id.remove(&event.event_instance_id()) {
                    cal_event.delete().map_err(CalendarError::from)?;
                }
            }
        }
    }

    Ok(())
}

async fn push_outgoing_changes(
    remote: &Remote,
    diff: &CalendarDiff,
    events_by_instance_id: &mut HashMap<EventInstanceId, CalendarEvent>,
    sync_bases: &mut Vec<Event>,
) -> Result<(), ConnectionError> {
    for change in diff.outgoing() {
        if let Some(remote_event) = remote.apply_change(change).await? {
            let returned_event = remote_event.event();

            // Sometimes provider overwrite the event's UID:
            let original_event_id = match change {
                EventChange::Create(event) => event.event_instance_id(),
                EventChange::Update { to, .. } => to.event_instance_id(),
                EventChange::Delete(_) => unreachable!("apply_change returns None for Delete"),
            };

            if let Some(cal_event) = events_by_instance_id.get_mut(&original_event_id) {
                cal_event
                    .update(returned_event.clone())
                    .map_err(CalendarError::from)?;
            }

            sync_bases.push(returned_event.clone());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::EventChange;
    use crate::event::{EventUid, XProperty};
    use crate::provider::mock_provider::MockProvider;
    use crate::test_utils::{
        incoming_create_diff, incoming_delete_diff, incoming_update_diff, outgoing_create_diff,
        outgoing_delete_diff, outgoing_update_diff, test_caldir, test_event, test_mock_provider,
        test_remote_config, test_remote_params,
    };
    use crate::{CalendarConfig, rpc};
    use pretty_assertions::assert_eq;

    fn calendar_config(read_only: Option<bool>) -> CalendarConfig {
        CalendarConfig::new(
            Some("Read-only test".to_string()),
            None,
            read_only,
            Some(test_remote_config("test-provider")),
        )
    }

    fn writable_connection() -> (tempfile::TempDir, MockProvider, Connection) {
        let (tmp, caldir) = test_caldir();
        let calendar = caldir
            .create_calendar("writable-cal", Some(calendar_config(Some(false))))
            .unwrap();
        let mock = test_mock_provider();
        let remote = Remote::new(mock.provider(), test_remote_params());
        let connection = Connection::new(calendar, remote);
        (tmp, mock, connection)
    }

    #[tokio::test]
    async fn diff_discards_outgoing_when_read_only() {
        let (_tmp, caldir) = test_caldir();
        let calendar = caldir
            .create_calendar("read-only-cal", Some(calendar_config(Some(true))))
            .unwrap();
        calendar.create_event(test_event()).unwrap();

        let mock = test_mock_provider();
        mock.reply::<rpc::ListEvents>(vec![]);
        let remote = Remote::new(mock.provider(), test_remote_params());

        let mut connection = Connection::new(calendar, remote);
        let diff = connection.diff(&DateRange::default()).await.unwrap();

        assert!(
            diff.outgoing().is_empty(),
            "outgoing should be empty for read-only calendars, got {:?}",
            diff.outgoing()
        );
    }

    #[tokio::test]
    async fn diff_includes_outgoing_when_not_read_only() {
        let (_tmp, caldir) = test_caldir();
        let calendar = caldir
            .create_calendar("writable-cal", Some(calendar_config(Some(false))))
            .unwrap();
        let event = test_event();
        calendar.create_event(event.clone()).unwrap();

        let mock = test_mock_provider();
        mock.reply::<rpc::ListEvents>(vec![]);
        let remote = Remote::new(mock.provider(), test_remote_params());

        let mut connection = Connection::new(calendar, remote);
        let diff = connection.diff(&DateRange::default()).await.unwrap();

        assert_eq!(diff.outgoing(), &[EventChange::Create(event)]);
    }

    #[tokio::test]
    async fn diff_backfills_base_for_in_sync_legacy_known_id() {
        let (_tmp, caldir) = test_caldir();
        let calendar = caldir
            .create_calendar("writable-cal", Some(calendar_config(Some(false))))
            .unwrap();
        let event = test_event();
        let id = event.event_instance_id();
        calendar.create_event(event.clone()).unwrap();

        // Legacy state: id is known, but no base was ever recorded.
        let state_dir = calendar.path().join(".caldir/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("known_event_ids"), id.to_string()).unwrap();
        let calendar = Calendar::load(calendar.path()).unwrap();
        assert_eq!(calendar.state().sync_base(&id), None);

        let mock = test_mock_provider();
        mock.reply::<rpc::ListEvents>(vec![event.clone()]);
        let remote = Remote::new(mock.provider(), test_remote_params());

        let mut connection = Connection::new(calendar, remote);
        let diff = connection.diff(&DateRange::default()).await.unwrap();

        assert!(diff.is_empty());
        let reloaded = Calendar::load(connection.local().path()).unwrap();
        assert_eq!(reloaded.state().sync_base(&id), Some(&event));
    }

    #[tokio::test]
    async fn diff_does_not_backfill_base_for_never_synced_pair() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::ListEvents>(vec![event]);
        connection.diff(&DateRange::default()).await.unwrap();

        let reloaded = Calendar::load(connection.local().path()).unwrap();
        assert!(!reloaded.state().synced_event_ids().contains(&id));
    }

    #[tokio::test]
    async fn apply_incoming_diff_creates_file_for_incoming_create() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();

        connection
            .apply_incoming_diff(&incoming_create_diff(event))
            .unwrap();

        let expected_path = connection
            .local()
            .path()
            .join("2026-01-01T1200__test-event.ics");
        assert!(expected_path.is_file());
    }

    #[tokio::test]
    async fn apply_incoming_diff_updates_file_for_incoming_update() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let from = test_event();
        let cal_event = connection.local().create_event(from.clone()).unwrap();
        let old_path = cal_event.path().to_path_buf();

        let mut to = from.clone();
        to.summary = Some("Updated Test Event".to_string());

        connection
            .apply_incoming_diff(&incoming_update_diff(from, to))
            .unwrap();

        let new_path = connection
            .local()
            .path()
            .join("2026-01-01T1200__updated-test-event.ics");
        assert!(new_path.is_file());
        assert!(!old_path.exists());
    }

    #[tokio::test]
    async fn apply_incoming_diff_deletes_file_for_incoming_delete() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();
        let cal_event = connection.local().create_event(event.clone()).unwrap();
        let path = cal_event.path().to_path_buf();

        connection
            .apply_incoming_diff(&incoming_delete_diff(event))
            .unwrap();

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn apply_incoming_diff_records_incoming_create_in_state() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();

        connection
            .apply_incoming_diff(&incoming_create_diff(event))
            .unwrap();

        assert!(connection.local().state().synced_event_ids().contains(&id));
    }

    #[tokio::test]
    async fn apply_incoming_diff_persists_base_to_disk() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();

        connection
            .apply_incoming_diff(&incoming_create_diff(event.clone()))
            .unwrap();

        let reloaded = Calendar::load(connection.local().path()).unwrap();
        assert_eq!(reloaded.state().sync_base(&id), Some(&event));
    }

    #[tokio::test]
    async fn apply_incoming_diff_does_not_record_deletes_in_state() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();
        connection.local().create_event(event.clone()).unwrap();

        connection
            .apply_incoming_diff(&incoming_delete_diff(event))
            .unwrap();

        assert!(!connection.local().state().synced_event_ids().contains(&id));
    }

    #[tokio::test]
    async fn apply_outgoing_diff_sends_create_event_for_outgoing_create() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::CreateEvent>(event.clone());
        connection
            .apply_outgoing_diff(&outgoing_create_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::CreateEvent>().event, event);
    }

    #[tokio::test]
    async fn apply_outgoing_diff_sends_update_event_for_outgoing_update() {
        let (_tmp, mock, mut connection) = writable_connection();
        let from = test_event();
        let mut to = from.clone();
        to.summary = Some("Updated".into());
        connection.local().create_event(to.clone()).unwrap();

        mock.reply::<rpc::UpdateEvent>(to.clone());
        connection
            .apply_outgoing_diff(&outgoing_update_diff(from, to.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::UpdateEvent>().event, to);
    }

    #[tokio::test]
    async fn apply_outgoing_diff_sends_delete_event_for_outgoing_delete() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();

        mock.reply::<rpc::DeleteEvent>(());
        connection
            .apply_outgoing_diff(&outgoing_delete_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::DeleteEvent>().event, event);
    }

    #[tokio::test]
    async fn apply_outgoing_diff_rewrites_local_with_canonical_event_from_provider() {
        let (_tmp, mock, mut connection) = writable_connection();
        let local = test_event();
        connection.local().create_event(local.clone()).unwrap();

        let mut canonical = local.clone();
        canonical.x_properties.push(XProperty::new(
            "X-GOOGLE-EVENT-ID".to_string(),
            "abc123".to_string(),
        ));
        mock.reply::<rpc::CreateEvent>(canonical.clone());

        connection
            .apply_outgoing_diff(&outgoing_create_diff(local))
            .await
            .unwrap();

        let reloaded = connection.local().events().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].event().x_properties, canonical.x_properties);
        assert_eq!(
            connection
                .local()
                .state()
                .sync_base(&canonical.event_instance_id()),
            Some(&canonical)
        );
    }

    #[tokio::test]
    async fn apply_outgoing_diff_rewrites_local_when_provider_reassigns_uid() {
        // Some providers (e.g. CalDAV servers) re-assign UID server-side. The
        // local file must still be rewritten with the provider's canonical
        // event, and the synced state must record the *returned* identity so
        // the next sync doesn't see a phantom delete + duplicate create.
        let (_tmp, mock, mut connection) = writable_connection();
        let local = test_event();
        let original_id = local.event_instance_id();
        connection.local().create_event(local.clone()).unwrap();

        let mut canonical = local.clone();
        canonical.uid = EventUid::new("provider-assigned-uid@example.com");
        let canonical_id = canonical.event_instance_id();
        assert_ne!(original_id, canonical_id);
        mock.reply::<rpc::CreateEvent>(canonical.clone());

        connection
            .apply_outgoing_diff(&outgoing_create_diff(local))
            .await
            .unwrap();

        let reloaded = connection.local().events().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].event().uid, canonical.uid);

        let synced = connection.local().state().synced_event_ids();
        assert!(synced.contains(&canonical_id));
        assert!(!synced.contains(&original_id));
    }

    #[tokio::test]
    async fn apply_outgoing_diff_records_base_for_outgoing_create() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::CreateEvent>(event.clone());
        connection
            .apply_outgoing_diff(&outgoing_create_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(connection.local().state().sync_base(&id), Some(&event));
    }

    #[tokio::test]
    async fn apply_outgoing_diff_persists_synced_ids_on_partial_success() {
        use crate::provider::transport::ProviderTransportError;
        use std::time::Duration;

        let (_tmp, mock, mut connection) = writable_connection();

        let event_a = test_event();
        let event_b = test_event();
        let id_a = event_a.event_instance_id();
        connection.local().create_event(event_a.clone()).unwrap();
        connection.local().create_event(event_b.clone()).unwrap();

        // First Create succeeds!
        // Second errors mid-loop!
        mock.reply::<rpc::CreateEvent>(event_a.clone());
        mock.reply_error(ProviderTransportError::Timeout(Duration::from_secs(1)));

        let diff = CalendarDiff::from_changes(
            vec![
                EventChange::Create(event_a.clone()),
                EventChange::Create(event_b),
            ],
            vec![],
        );

        let result = connection.apply_outgoing_diff(&diff).await;

        assert!(
            result.is_err(),
            "expected the second create to propagate an error",
        );

        let reloaded = Calendar::load(connection.local().path()).unwrap();

        assert_eq!(
            reloaded.state().sync_base(&id_a),
            Some(&event_a),
            "event A's base should survive a later push failure",
        );
    }

    #[tokio::test]
    async fn discard_outgoing_diff_deletes_file_for_outgoing_create() {
        let (_tmp, _mock, connection) = writable_connection();
        let event = test_event();
        let cal_event = connection.local().create_event(event.clone()).unwrap();
        let path = cal_event.path().to_path_buf();

        connection
            .discard_outgoing_diff(&outgoing_create_diff(event))
            .unwrap();

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn discard_outgoing_diff_reverts_local_update_to_remote_version() {
        let (_tmp, _mock, connection) = writable_connection();
        let original = test_event();
        let mut modified = original.clone();
        modified.summary = Some("Locally Edited".to_string());
        connection.local().create_event(modified.clone()).unwrap();

        connection
            .discard_outgoing_diff(&outgoing_update_diff(original.clone(), modified))
            .unwrap();

        let reloaded = connection.local().events().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].event().summary, original.summary);
    }

    #[tokio::test]
    async fn discard_outgoing_diff_recreates_file_for_outgoing_delete() {
        let (_tmp, _mock, connection) = writable_connection();
        let event = test_event();

        connection
            .discard_outgoing_diff(&outgoing_delete_diff(event))
            .unwrap();

        let expected_path = connection
            .local()
            .path()
            .join("2026-01-01T1200__test-event.ics");
        assert!(expected_path.is_file());
    }
}
