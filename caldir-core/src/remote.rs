mod config;

use crate::provider::ProviderError;
use crate::{Event, Provider, rpc};
pub use config::{RemoteConfig, RemoteConfigParams};

/// provider with config should resolve to a unique remote
pub struct Remote {
    provider: Provider,
    params: RemoteConfigParams,
}

impl Remote {
    pub fn new(provider: Provider, params: RemoteConfigParams) -> Self {
        Self { provider, params }
    }

    pub async fn create_event(&self, event: Event) -> Result<Event, ProviderError> {
        self.provider
            .call(rpc::CreateEvent {
                remote: self.params.clone(),
                event,
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventTime;
    use crate::provider::mock_provider::MockProvider;
    use crate::rpc::CreateEvent;

    fn hooli_remote_config_params() -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        params
    }

    #[tokio::test]
    async fn create_event_sends_request_and_parses_response() {
        let returned_server_event = Event::new(
            "Server-side title",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let mock = MockProvider::new("hooli")
            .expect::<CreateEvent>()
            .reply(returned_server_event.clone());

        let remote = Remote::new(mock.provider(), hooli_remote_config_params());

        let local_event = Event::new(
            "Local title",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()),
        );
        let result = remote.create_event(local_event.clone()).await.unwrap();

        assert_eq!(result.uid, returned_server_event.uid);
        assert_eq!(result.summary.as_deref(), Some("Server-side title"));

        let req = mock.captured_request::<CreateEvent>();
        assert_eq!(
            req.remote.get("hooli_account"),
            Some(&toml::Value::String("user@hmail.com".to_string())),
        );
        assert_eq!(req.event.uid, local_event.uid);
        assert_eq!(req.event.summary.as_deref(), Some("Local title"));
    }
}
