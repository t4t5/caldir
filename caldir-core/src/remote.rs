mod config;
mod error;
mod event;

use crate::diff::{CalendarDiff, EventChange};
use crate::provider::ProviderError;
use crate::{DateRange, Event, Provider, rpc};

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

    pub async fn list_events(&self, range: &DateRange) -> Result<Vec<RemoteEvent>, RemoteError> {
        let (from, to) = range.to_rfc3339();
        let events = self
            .provider
            .call(rpc::ListEvents {
                remote: self.params.clone(),
                from,
                to,
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
                EventChange::Update { from, to } => {
                    let merged = to.clone().with_x_properties_merged_from(from);
                    self.update_event(merged).await?;
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
    async fn apply_diff_update_merges_x_property_params_from_remote() {
        // Local copy was written by an old parser that dropped params;
        // remote still carries them. Push must send the union so we don't
        // strip provider-managed metadata.
        let (mock, remote) = test_remote();

        let mut from = test_event();
        from.x_properties = vec![crate::event::XProperty {
            name: "X-APPLE-STRUCTURED-LOCATION".to_string(),
            value: "geo:51.47,-0.45".to_string(),
            params: vec![("X-TITLE".to_string(), "London Heathrow".to_string())],
        }];

        let mut to = from.clone();
        to.summary = Some("Updated".into());
        to.x_properties = vec![crate::event::XProperty::new(
            "X-APPLE-STRUCTURED-LOCATION",
            "geo:51.47,-0.45",
        )];

        mock.reply::<rpc::UpdateEvent>(to.clone());

        remote
            .apply_diff(&outgoing_update_diff(from, to))
            .await
            .unwrap();

        let captured = mock.captured_request::<rpc::UpdateEvent>().event;
        assert_eq!(captured.x_properties.len(), 1);
        assert_eq!(
            captured.x_properties[0].params,
            vec![("X-TITLE".to_string(), "London Heathrow".to_string())]
        );
    }

    #[tokio::test]
    async fn apply_diff_sends_delete_event_for_outgoing_delete() {
        let (mock, remote) = test_remote();
        let event = test_event();
        mock.reply::<rpc::DeleteEvent>(());

        remote
            .apply_diff(&outgoing_delete_diff(event.clone()))
            .await
            .unwrap();

        assert_eq!(mock.captured_request::<rpc::DeleteEvent>().event, event);
    }
}
