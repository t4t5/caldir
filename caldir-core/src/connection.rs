mod error;

use std::collections::HashMap;

use crate::calendar::CalendarError;
use crate::diff::EventChange;
use crate::event::EventInstanceId;
use crate::{Calendar, CalendarDiff, CalendarEvent, DateRange, Remote};
use error::ConnectionError;

use crate::calendar::SyncedEventIds;

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

        let mut diff = CalendarDiff::compute(local_events, remote_events, synced_ids, range);

        if self.read_only() {
            diff.discard_outgoing();
        }

        Ok(diff)
    }

    /// Apply outgoing changes to the remote, then rewrite local files with the
    /// canonical events the provider returned so subsequent diffs don't see
    /// spurious updates (e.g. when the provider adds X-GOOGLE-EVENT-ID).
    pub async fn push(&mut self, diff: &CalendarDiff) -> Result<(), ConnectionError> {
        let mut events_by_instance_id: HashMap<EventInstanceId, CalendarEvent> = self
            .local
            .events()?
            .into_iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        let mut synced_ids = Vec::new();

        for change in diff.outgoing() {
            match change {
                EventChange::Create(event) => {
                    let remote_event = self.remote.create_event(event.clone()).await?;
                    let canonical = remote_event.event();
                    if let Some(cal_event) =
                        events_by_instance_id.get_mut(&event.event_instance_id())
                    {
                        cal_event
                            .update(canonical.clone())
                            .map_err(CalendarError::from)?;
                    }
                    synced_ids.push(canonical.event_instance_id());
                }
                EventChange::Update { to, .. } => {
                    let remote_event = self.remote.update_event(to.clone()).await?;
                    let canonical = remote_event.event();
                    if let Some(cal_event) = events_by_instance_id.get_mut(&to.event_instance_id())
                    {
                        cal_event
                            .update(canonical.clone())
                            .map_err(CalendarError::from)?;
                    }
                    synced_ids.push(canonical.event_instance_id());
                }
                EventChange::Delete(event) => {
                    self.remote.delete_event(event.clone()).await?;
                }
            }
        }

        self.local.record_synced_ids(synced_ids)?;

        Ok(())
    }

    fn synced_event_ids(&self) -> &SyncedEventIds {
        self.local().state().synced_event_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::EventChange;
    use crate::event::XProperty;
    use crate::provider::mock_provider::MockProvider;
    use crate::test_utils::{
        outgoing_create_diff, outgoing_delete_diff, outgoing_update_diff, test_caldir, test_event,
        test_mock_provider, test_remote_config, test_remote_params,
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
    async fn push_sends_create_event_for_outgoing_create() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::CreateEvent>(event.clone());
        connection
            .push(&outgoing_create_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::CreateEvent>().event, event);
    }

    #[tokio::test]
    async fn push_sends_update_event_for_outgoing_update() {
        let (_tmp, mock, mut connection) = writable_connection();
        let from = test_event();
        let mut to = from.clone();
        to.summary = Some("Updated".into());
        connection.local().create_event(to.clone()).unwrap();

        mock.reply::<rpc::UpdateEvent>(to.clone());
        connection
            .push(&outgoing_update_diff(from, to.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::UpdateEvent>().event, to);
    }

    #[tokio::test]
    async fn push_sends_delete_event_for_outgoing_delete() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();

        mock.reply::<rpc::DeleteEvent>(());
        connection
            .push(&outgoing_delete_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::DeleteEvent>().event, event);
    }

    #[tokio::test]
    async fn push_rewrites_local_with_canonical_event_from_provider() {
        let (_tmp, mock, mut connection) = writable_connection();
        let local = test_event();
        connection.local().create_event(local.clone()).unwrap();

        let mut canonical = local.clone();
        canonical.x_properties.push(XProperty::new(
            "X-GOOGLE-EVENT-ID".to_string(),
            "abc123".to_string(),
        ));
        mock.reply::<rpc::CreateEvent>(canonical.clone());

        connection.push(&outgoing_create_diff(local)).await.unwrap();

        let reloaded = connection.local().events().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].event().x_properties, canonical.x_properties);
    }

    #[tokio::test]
    async fn push_records_synced_id_for_outgoing_create() {
        let (_tmp, mock, mut connection) = writable_connection();
        let event = test_event();
        let id = event.event_instance_id();
        connection.local().create_event(event.clone()).unwrap();

        mock.reply::<rpc::CreateEvent>(event.clone());
        connection.push(&outgoing_create_diff(event)).await.unwrap();

        assert!(connection.local().state().synced_event_ids().contains(&id));
    }
}
