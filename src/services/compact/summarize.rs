//! Core compaction: summarize a conversation with a model call.
//!
//! Ported from the `compactConversation` function in `ref/services/compact/compact.ts`.
//! The main entry point is `compact_messages`, which sends the conversation to the
//! model with a compaction prompt and returns a structured `CompactionResult`.

use futures::StreamExt;
use crate::api::request::ApiMessage;
use crate::api::streaming::ContentDelta;
use crate::api::{ApiClient, ApiError, CreateMessageRequest, StreamEvent};
use crate::types::Message;

use super::prompt::{format_compact_summary, get_compact_prompt};

// ---------------------------------------------------------------------------
// CompactionResult
// ---------------------------------------------------------------------------

/// Result of a conversation compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The formatted summary text produced by the model.
    pub summary: String,

    /// Number of messages preserved (kept after the summary).
    pub preserved_count: usize,

    /// Number of messages removed (replaced by the summary).
    pub removed_count: usize,

    /// Estimated tokens saved by compaction.
    pub tokens_saved: u64,

    /// Token count of the conversation before compaction (from API usage).
    pub pre_compact_token_count: Option<u64>,

    /// Estimated token count of the conversation after compaction.
    pub post_compact_token_count: u64,
}

// ---------------------------------------------------------------------------
// compact_messages
// ---------------------------------------------------------------------------

/// Compact a conversation by summarising the older messages with a model call.
///
/// # Arguments
///
/// * `messages` -- The full conversation to compact.
/// * `model` -- The model identifier to use for the summarisation call.
/// * `client` -- An `ApiClient` to make the API request.
///
/// The function:
/// 1. Builds a compaction prompt (from `prompt.rs`).
/// 2. Converts the conversation messages into API-format messages.
/// 3. Sends a streaming request with `max_tokens` capped for summaries.
/// 4. Collects the streamed text into the final summary.
/// 5. Strips the `<analysis>` scratchpad and returns a `CompactionResult`.
pub async fn compact_messages(
    messages: &[Message],
    model: &str,
    client: &ApiClient,
) -> Result<CompactionResult, CompactError> {
    compact_messages_with_options(messages, model, client, None).await
}

/// Like `compact_messages` but accepts optional custom instructions.
pub async fn compact_messages_with_options(
    messages: &[Message],
    model: &str,
    client: &ApiClient,
    custom_instructions: Option<&str>,
) -> Result<CompactionResult, CompactError> {
    if messages.is_empty() {
        return Err(CompactError::EmptyConversation);
    }

    // Build the compact prompt that will be appended as a user message.
    let compact_prompt = get_compact_prompt(custom_instructions);

    // Convert conversation messages to API format.
    let mut api_messages: Vec<ApiMessage> = Vec::new();
    for msg in messages {
        match msg {
            Message::User(u) => {
                let text = extract_text_from_content(&u.content);
                if !text.is_empty() {
                    api_messages.push(ApiMessage::user(&text));
                }
            }
            Message::Assistant(a) => {
                let text = extract_text_from_content(&a.content);
                if !text.is_empty() {
                    api_messages.push(ApiMessage::assistant(&text));
                }
            }
            // System, progress, attachment, etc. are skipped for the compaction call.
            _ => {}
        }
    }

    if api_messages.is_empty() {
        return Err(CompactError::EmptyConversation);
    }

    // Append the compaction instruction as a trailing user message.
    api_messages.push(ApiMessage::user(&compact_prompt));

    let request = CreateMessageRequest {
        model: model.to_owned(),
        max_tokens: Some(super::MAX_OUTPUT_TOKENS_FOR_SUMMARY),
        messages: api_messages,
        temperature: Some(0.0),
        top_p: None,
        stop: None,
        stream: true,
        tools: None,
        tool_choice: None,
    };

    // Send streaming request and accumulate the response text.
    let stream = client
        .create_message_stream(request)
        .await
        .map_err(CompactError::Api)?;

    let mut summary_text = String::new();
    futures::pin_mut!(stream);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => match event {
                StreamEvent::ContentBlockDelta { delta, .. } => {
                    if let ContentDelta::TextDelta { text } = delta {
                        summary_text.push_str(&text);
                    }
                }
                StreamEvent::Error { error } => {
                    return Err(CompactError::Api(error));
                }
                // Ignore other events (ping, message_start, etc.).
                _ => {}
            },
            Err(e) => {
                return Err(CompactError::Api(e));
            }
        }
    }

    if summary_text.trim().is_empty() {
        return Err(CompactError::EmptySummary);
    }

    let formatted = format_compact_summary(&summary_text);
    let estimated_summary_tokens = rough_token_estimate(&formatted);
    let estimated_original_tokens = estimate_message_tokens(messages);

    Ok(CompactionResult {
        summary: formatted,
        preserved_count: 0,
        removed_count: messages.len(),
        tokens_saved: estimated_original_tokens.saturating_sub(estimated_summary_tokens),
        pre_compact_token_count: Some(estimated_original_tokens),
        post_compact_token_count: estimated_summary_tokens,
    })
}

// ---------------------------------------------------------------------------
// CompactError
// ---------------------------------------------------------------------------

/// Errors that can occur during compaction.
#[derive(Debug, thiserror::Error)]
pub enum CompactError {
    #[error("conversation is empty, nothing to compact")]
    EmptyConversation,

    #[error("model produced an empty summary")]
    EmptySummary,

    #[error("user aborted compaction")]
    UserAbort,

    #[error("API error during compaction: {0}")]
    Api(#[from] ApiError),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract plain text from content blocks (text blocks only).
fn extract_text_from_content(content: &[crate::types::ContentBlock]) -> String {
    let mut parts = Vec::new();
    for block in content {
        match block {
            crate::types::ContentBlock::Text { text } => {
                parts.push(text.as_str());
            }
            crate::types::ContentBlock::ToolUse { name, .. } => {
                parts.push(name.as_str());
            }
            crate::types::ContentBlock::ToolResult { content, .. } => match content {
                crate::types::ToolResultContent::Text(t) => {
                    parts.push(t.as_str());
                }
                crate::types::ToolResultContent::Blocks(blocks) => {
                    for b in blocks {
                        if let crate::types::ContentBlock::Text { text } = b {
                            parts.push(text.as_str());
                        }
                    }
                }
            },
            _ => {}
        }
    }
    parts.join("\n")
}

/// Rough token estimate: ~4 characters per token, padded by 4/3 to be
/// conservative. Matches the TypeScript `roughTokenCountEstimation`.
fn rough_token_estimate(text: &str) -> u64 {
    let base = (text.len() as f64 / 4.0).ceil() as u64;
    (base as f64 * 4.0 / 3.0).ceil() as u64
}

/// Estimate total tokens for a slice of messages. Walks text blocks only.
pub(crate) fn estimate_message_tokens(messages: &[Message]) -> u64 {
    let mut total: u64 = 0;
    for msg in messages {
        let content = match msg {
            Message::User(u) => &u.content,
            Message::Assistant(a) => &a.content,
            _ => continue,
        };
        for block in content {
            match block {
                crate::types::ContentBlock::Text { text } => {
                    total += rough_token_estimate(text);
                }
                crate::types::ContentBlock::ToolResult { content, .. } => match content {
                    crate::types::ToolResultContent::Text(t) => {
                        total += rough_token_estimate(t);
                    }
                    crate::types::ToolResultContent::Blocks(blocks) => {
                        for b in blocks {
                            if let crate::types::ContentBlock::Text { text } = b {
                                total += rough_token_estimate(text);
                            }
                        }
                    }
                },
                crate::types::ContentBlock::Thinking { thinking, .. } => {
                    total += rough_token_estimate(thinking);
                }
                crate::types::ContentBlock::RedactedThinking { data } => {
                    total += rough_token_estimate(data);
                }
                crate::types::ContentBlock::ToolUse { name, input, .. } => {
                    total += rough_token_estimate(name);
                    total += rough_token_estimate(&input.to_string());
                }
                _ => {}
            }
        }
    }
    // Pad by 4/3 to be conservative, matching the TS implementation.
    (total as f64 * 4.0 / 3.0).ceil() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rough_estimate_nonzero() {
        let estimate = rough_token_estimate("Hello, world! This is a test.");
        assert!(estimate > 0);
    }

    #[test]
    fn empty_messages_returns_zero() {
        assert_eq!(estimate_message_tokens(&[]), 0);
    }
}
