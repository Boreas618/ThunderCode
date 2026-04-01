//! Provider-neutral API client for OpenAI-compatible endpoints.

pub mod client;
pub mod errors;
pub mod models;
pub mod proxy;
pub mod request;
pub mod retry;
pub mod streaming;

pub use client::ApiClient;
pub use errors::ApiError;
pub use models::{get_model_info, resolve_model_name, ModelInfo};
pub use request::{ApiMessage, CreateMessageRequest, ToolDefinition};
pub use retry::{with_retry, RetryConfig};
pub use streaming::{parse_sse_stream, ContentDelta, StreamEvent};
