mod config;
mod error;
mod event;

use crate::diff::{CalendarDiff, EventChange};
use crate::provider::ProviderError;
use crate::{Event, Provider, rpc};

pub use config::{RemoteConfig, RemoteConfigParams};
pub(crate) use error::RemoteError;
pub use event::RemoteEvent;

/// provider with config should resolve to a unique remote
pub struct Remote {
    provider: Provider,
    params: RemoteConfigParams,
}

impl Remote {
    pub fn new(provider: Provider, params: RemoteConfigParams) -> Self {
        Self { provider, params }
    }

    pub async fn list_events(&self) -> Result<Vec<RemoteEvent>, RemoteError> {
        let events = self
            .provider
            .call(rpc::ListEvents {
                remote: self.params.clone(),
            })
            .await?
            .into_iter()
            .map(RemoteEvent::new)
            .collect();

        Ok(events)
    }

    pub async fn create_event(&self, event: Event) -> Result<RemoteEvent, RemoteError> {
        let event = self
            .provider
            .call(rpc::CreateEvent {
                remote: self.params.clone(),
                event,
            })
            .await?;

        Ok(RemoteEvent::new(event))
    }

    pub async fn delete_event(&self, event: Event) -> Result<(), RemoteError> {
        self.provider
            .call(rpc::DeleteEvent {
                remote: self.params.clone(),
                event,
            })
            .await?;

        Ok(())
    }

    pub async fn update_event(&self, event: Event) -> Result<RemoteEvent, RemoteError> {
        let event = self
            .provider
            .call(rpc::UpdateEvent {
                remote: self.params.clone(),
                event,
            })
            .await?;

        Ok(RemoteEvent::new(event))
    }

    pub async fn apply_diff(&self, diff: &CalendarDiff) -> Result<(), RemoteError> {
        for change in diff.outgoing() {
            match change {
                EventChange::Create(event) => {
                    self.create_event(event.clone()).await?;
                }
                EventChange::Update { to, .. } => {
                    self.update_event(to.clone()).await?;
                }
                EventChange::Delete(event) => {
                    self.delete_event(event.clone()).await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        outgoing_create_diff, outgoing_delete_diff, outgoing_update_diff, test_event, test_remote,
    };
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn apply_diff_sends_create_event_for_outgoing_create() {
        let (mock, remote) = test_remote();
        let event = test_event();
        mock.reply::<rpc::CreateEvent>(event.clone());

        remote
            .apply_diff(&outgoing_create_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::CreateEvent>().event, event);
    }

    #[tokio::test]
    async fn apply_diff_sends_update_event_for_outgoing_update() {
        let (mock, remote) = test_remote();
        let from = test_event();
        let mut to = from.clone();
        to.summary = Some("Updated".into());
        mock.reply::<rpc::UpdateEvent>(to.clone());

        remote
            .apply_diff(&outgoing_update_diff(from, to.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::UpdateEvent>().event, to);
    }

    #[tokio::test]
    async fn apply_diff_sends_delete_event_for_outgoing_delete() {
        let (mock, remote) = test_remote();
        let event = test_event();
        mock.reply::<rpc::DeleteEvent>(event.clone());

        remote
            .apply_diff(&outgoing_delete_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::DeleteEvent>().event, event);
    }
}
