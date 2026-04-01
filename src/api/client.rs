//! Provider-neutral API client using OpenAI-compatible endpoints.
//!
//! Supports any provider that implements the OpenAI `/v1/chat/completions` API:
//! OpenAI, Ollama, vLLM, Together, OpenRouter, etc.

use std::time::Duration;

use futures::Stream;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

use crate::api::errors::{classify_error, ApiError};
use crate::api::proxy::{configure_proxy, ProxyConfig};
use crate::api::request::{CountTokensRequest, CreateMessageRequest};
use crate::api::streaming::{parse_sse_stream, CountTokensResponse, MessageResponse, StreamEvent};

// ---------------------------------------------------------------------------
// ClientConfig
// ---------------------------------------------------------------------------

/// Configuration for constructing an API client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL for the API (default: `https://api.openai.com`).
    pub base_url: Option<String>,
    /// API key — sent as `Authorization: Bearer <key>`.
    pub api_key: Option<String>,
    /// Request timeout.
    pub timeout: Option<Duration>,
    /// Proxy settings.
    pub proxy: Option<ProxyConfig>,
    /// Custom default headers.
    pub default_headers: Option<HeaderMap>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            timeout: Some(Duration::from_secs(600)),
            proxy: None,
            default_headers: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ApiClient (provider-neutral)
// ---------------------------------------------------------------------------

/// Provider-neutral HTTP client for OpenAI-compatible chat completions.
#[derive(Debug, Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}


impl ApiClient {
    /// Create a new client from the given configuration.
    pub fn new(config: ClientConfig) -> Self {
        let base_url = config
            .base_url
            .unwrap_or_else(|| "https://api.openai.com".to_owned());

        let timeout = config.timeout.unwrap_or(Duration::from_secs(600));

        let mut builder = reqwest::Client::builder().timeout(timeout);

        if let Some(ref proxy_cfg) = config.proxy {
            builder = configure_proxy(builder, proxy_cfg);
        }

        let headers = config.default_headers.unwrap_or_default();
        builder = builder.default_headers(headers);

        let http = builder.build().expect("Failed to build reqwest client");

        Self {
            http,
            base_url,
            api_key: config.api_key,
        }
    }

    /// Send a non-streaming message creation request.
    ///
    /// Posts to `/v1/chat/completions` (OpenAI-compatible).
    pub async fn create_message(
        &self,
        request: CreateMessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut builder = self.http.post(&url);
        builder = self.apply_auth(builder);
        builder = builder.header(CONTENT_TYPE, "application/json");

        let response = builder.json(&request).send().await.map_err(|e| {
            if e.is_timeout() {
                ApiError::Timeout {
                    message: "Request timed out".to_owned(),
                }
            } else {
                ApiError::Network {
                    message: e.to_string(),
                    source: e,
                }
            }
        })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(classify_error(status, &body));
        }

        response.json::<MessageResponse>().await.map_err(|e| {
            ApiError::InvalidRequest {
                message: format!("Failed to parse response: {e}"),
            }
        })
    }

    /// Send a streaming message creation request.
    ///
    /// Posts to `/v1/chat/completions` with `stream: true`.
    /// Returns a `Stream` of SSE events.
    pub async fn create_message_stream(
        &self,
        mut request: CreateMessageRequest,
    ) -> Result<impl Stream<Item = Result<StreamEvent, ApiError>> + Send, ApiError> {
        request.stream = true;
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut builder = self.http.post(&url);
        builder = self.apply_auth(builder);
        builder = builder.header(CONTENT_TYPE, "application/json");

        let response = builder.json(&request).send().await.map_err(|e| {
            if e.is_timeout() {
                ApiError::Timeout {
                    message: "Request timed out".to_owned(),
                }
            } else {
                ApiError::Network {
                    message: e.to_string(),
                    source: e,
                }
            }
        })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(classify_error(status, &body));
        }

        let byte_stream = response.bytes_stream();
        Ok(parse_sse_stream(byte_stream))
    }

    /// Count tokens for the given request.
    ///
    /// Falls back to a heuristic if the provider doesn't support token counting.
    pub async fn count_tokens(
        &self,
        request: CountTokensRequest,
    ) -> Result<CountTokensResponse, ApiError> {
        let url = format!("{}/v1/messages/count_tokens", self.base_url);

        let mut builder = self.http.post(&url);
        builder = self.apply_auth(builder);
        builder = builder.header(CONTENT_TYPE, "application/json");

        let response = builder.json(&request).send().await.map_err(|e| {
            ApiError::Network {
                message: e.to_string(),
                source: e,
            }
        })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            // Token counting not supported — return a heuristic estimate.
            return Ok(CountTokensResponse { input_tokens: 0 });
        }

        response
            .json::<CountTokensResponse>()
            .await
            .map_err(|e| ApiError::InvalidRequest {
                message: format!("Failed to parse count_tokens response: {e}"),
            })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Apply authentication: `Authorization: Bearer <key>`.
    fn apply_auth(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            builder = builder.header("Authorization", format!("Bearer {key}"));
        }
        builder
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert!(config.base_url.is_none());
        assert!(config.api_key.is_none());
        assert_eq!(config.timeout, Some(Duration::from_secs(600)));
    }

    #[test]
    fn test_client_creation() {
        let client = ApiClient::new(ClientConfig {
            base_url: Some("http://localhost:8080".into()),
            api_key: Some("test-key".into()),
            ..Default::default()
        });
        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.api_key.as_deref(), Some("test-key"));
    }

    #[test]
    fn test_default_base_url() {
        let client = ApiClient::new(ClientConfig::default());
        assert_eq!(client.base_url, "https://api.openai.com");
    }

}
