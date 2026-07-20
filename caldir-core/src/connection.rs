mod error;

use std::collections::{HashMap, HashSet};

use crate::calendar::CalendarError;
use crate::diff::EventChange;
use crate::event::EventInstanceId;
use crate::{Calendar, CalendarDiff, CalendarEvent, DateRange, Remote};
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

    pub async fn diff(&self, range: &DateRange) -> Result<CalendarDiff, ConnectionError> {
        let local_events = self.local().events()?;
        let remote_events = self.remote().list_events(range).await?;

        let synced_ids = self.synced_event_ids();

        let mut diff = CalendarDiff::compute(local_events, remote_events, &synced_ids, range);

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

        let mut synced_ids = Vec::new();

        // Same partial-failure flush pattern as `apply_outgoing_diff`: a
        // local-fs error mid-loop must not drop the ids of changes we've
        // already applied to disk.
        let loop_result = pull_incoming_changes(
            &self.local,
            diff,
            &mut events_by_instance_id,
            &mut synced_ids,
        );

        let record_result = self.local.record_synced_ids(synced_ids);

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

        let mut synced_ids = Vec::new();

        // Handles mid-loop errors gracefully
        let loop_result = push_outgoing_changes(
            &self.remote,
            diff,
            &mut events_by_instance_id,
            &mut synced_ids,
        )
        .await;

        let record_result = self.local.record_synced_ids(synced_ids);

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

    fn synced_event_ids(&self) -> HashSet<EventInstanceId> {
        self.local().state().synced_event_ids()
    }
}

fn pull_incoming_changes(
    local: &Calendar,
    diff: &CalendarDiff,
    events_by_instance_id: &mut HashMap<EventInstanceId, CalendarEvent>,
    synced_ids: &mut Vec<EventInstanceId>,
) -> Result<(), ConnectionError> {
    for change in diff.incoming() {
        match change {
            EventChange::Create(event) => {
                let cal_event = local.create_event(event.clone())?;
                let id = cal_event.event().event_instance_id();
                events_by_instance_id.insert(id.clone(), cal_event);
                synced_ids.push(id);
            }
            EventChange::Update { to, .. } => {
                if let Some(cal_event) = events_by_instance_id.get_mut(&to.event_instance_id()) {
                    cal_event.update(to.clone()).map_err(CalendarError::from)?;
                }
                synced_ids.push(to.event_instance_id());
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
    synced_ids: &mut Vec<EventInstanceId>,
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

            synced_ids.push(returned_event.event_instance_id());
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

        let connection = Connection::new(calendar, remote);
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

        let connection = Connection::new(calendar, remote);
        let diff = connection.diff(&DateRange::default()).await.unwrap();

        assert_eq!(diff.outgoing(), &[EventChange::Create(event)]);
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
    async fn apply_incoming_diff_persists_state_to_disk() {
        let (_tmp, _mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();

        connection
            .apply_incoming_diff(&incoming_create_diff(event))
            .unwrap();

        let reloaded = Calendar::load(connection.local().path()).unwrap();
        assert!(reloaded.state().synced_event_ids().contains(&id));
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
    async fn apply_outgoing_diff_records_synced_id_for_outgoing_create() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::CreateEvent>(event.clone());
        connection
            .apply_outgoing_diff(&outgoing_create_diff(event))
            .await
            .unwrap();

        assert!(connection.local().state().synced_event_ids().contains(&id));
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
            vec![EventChange::Create(event_a), EventChange::Create(event_b)],
            vec![],
        );

        let result = connection.apply_outgoing_diff(&diff).await;

        assert!(
            result.is_err(),
            "expected the second create to propagate an error",
        );

        let reloaded = Calendar::load(connection.local().path()).unwrap();

        // We should still have saved the instance ID for the event that was pushed!
        assert!(
            reloaded.state().synced_event_ids().contains(&id_a),
            "known_event_ids on disk should contain event A's id after a partial-success push",
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
