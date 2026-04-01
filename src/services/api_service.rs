//! API service wrapper.
//!
//! A thin wrapper around `ApiClient` that provides the streaming query
//! interface used by the main REPL loop. Ported from ref/services/api/primary.ts`.

use futures::Stream;

use crate::api::client::ClientConfig;
use crate::api::{ApiClient, ApiError, CreateMessageRequest, StreamEvent};

// ---------------------------------------------------------------------------
// ApiService
// ---------------------------------------------------------------------------

/// High-level wrapper around `ApiClient` for the main query loop.
///
/// This centralises the streaming call so that callers (the REPL, compact,
/// forked agents, etc.) have a single entry point for model queries.
pub struct ApiService {
    client: ApiClient,
}

impl ApiService {
    /// Create a new `ApiService` from the given client configuration.
    pub fn new(config: ClientConfig) -> Self {
        Self {
            client: ApiClient::new(config),
        }
    }

    /// Create a new `ApiService` wrapping an existing `ApiClient`.
    pub fn from_client(client: ApiClient) -> Self {
        Self { client }
    }

    /// Send a streaming message creation request.
    ///
    /// Returns an async `Stream` of `StreamEvent`s. The caller drives the
    /// stream to completion, accumulating text deltas, tool-use blocks, and
    /// thinking tokens as they arrive.
    pub async fn query_model_with_streaming(
        &self,
        request: CreateMessageRequest,
    ) -> Result<impl Stream<Item = Result<StreamEvent, ApiError>>, ApiError> {
        self.client.create_message_stream(request).await
    }

    /// Send a non-streaming (blocking) message creation request.
    pub async fn query_model(
        &self,
        request: CreateMessageRequest,
    ) -> Result<crate::api::streaming::MessageResponse, ApiError> {
        self.client.create_message(request).await
    }

    /// Count tokens for the given request without generating a response.
    pub async fn count_tokens(
        &self,
        request: crate::api::request::CountTokensRequest,
    ) -> Result<crate::api::streaming::CountTokensResponse, ApiError> {
        self.client.count_tokens(request).await
    }

    /// Access the underlying client (e.g. for compact or forked agents).
    pub fn client(&self) -> &ApiClient {
        &self.client
    }
}

impl std::fmt::Debug for ApiService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiService")
            .field("client", &self.client)
            .finish()
    }
}
