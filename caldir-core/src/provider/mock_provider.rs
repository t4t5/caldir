use super::transport::ProviderTransport;
use super::transport::ProviderTransportError;
use super::transport::mock_transport::MockTransport;
use super::{Provider, ProviderSlug};
use crate::rpc::{Request, Rpc};
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Test helper for stubbing a `Provider`'s transport with typed RPC expectations.
pub(crate) struct MockProvider {
    slug: ProviderSlug,
    transport: Arc<MockTransport>,
}

impl MockProvider {
    pub(crate) fn new(slug: impl Into<ProviderSlug>) -> Self {
        Self {
            slug: slug.into(),
            transport: Arc::new(MockTransport::empty()),
        }
    }

    /// Stub the next RPC call to return `response` (typed by `C::Response`).
    pub(crate) fn reply<C: Rpc>(&self, response: C::Response) {
        let envelope = serde_json::json!({
            "status": "success",
            "data": response,
        });
        self.transport.set_response(envelope.to_string());
    }

    /// Stub the next RPC call to fail with a transport-level error.
    pub(crate) fn reply_error(&self, error: ProviderTransportError) {
        self.transport.set_error(error);
    }

    pub(crate) fn provider(&self) -> Provider {
        Provider::with_transport(
            self.slug.clone(),
            self.transport.clone() as Arc<dyn ProviderTransport>,
        )
    }

    /// Decode the captured request as `C`, asserting the wire method matches `C::METHOD`.
    pub(crate) fn captured_request<C: Rpc + DeserializeOwned>(&self) -> C {
        let raw = self
            .transport
            .captured_request()
            .expect("no request was sent to the mock");
        let request: Request =
            serde_json::from_str(&raw).expect("captured request was not a valid RPC envelope");
        assert_eq!(
            request.method,
            C::METHOD,
            "expected method {:?} but request used {:?}",
            C::METHOD,
            request.method,
        );
        serde_json::from_value(request.params)
            .expect("captured params did not deserialize as the expected RPC type")
    }
}
