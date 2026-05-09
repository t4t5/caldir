use super::transport::ProviderTransport;
use super::transport::mock_transport::MockTransport;
use super::{Provider, ProviderSlug};
use crate::rpc::{Request, Rpc};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
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
            // Replaced once `.expect::<C>().reply(...)` runs.
            transport: Arc::new(MockTransport::with_response("")),
        }
    }

    pub(crate) fn expect<C: Rpc>(self) -> Expect<C> {
        Expect {
            mock: self,
            _phantom: PhantomData,
        }
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

pub(crate) struct Expect<C> {
    mock: MockProvider,
    _phantom: PhantomData<C>,
}

impl<C: Rpc> Expect<C> {
    pub(crate) fn reply(self, response: C::Response) -> MockProvider {
        let envelope = serde_json::json!({
            "status": "success",
            "data": response,
        });
        MockProvider {
            slug: self.mock.slug,
            transport: Arc::new(MockTransport::with_response(envelope.to_string())),
        }
    }
}
