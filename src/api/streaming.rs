//! SSE stream parsing for OpenAI-compatible chat completions.
//!
//! OpenAI streaming format:
//! ```text
//! data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"},"index":0}]}
//!
//! data: [DONE]
//! ```

use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::api::errors::ApiError;
use crate::api::request::ToolCall;

// ---------------------------------------------------------------------------
// StreamEvent (provider-neutral)
// ---------------------------------------------------------------------------

/// A parsed event from an SSE stream.
#[derive(Debug)]
pub enum StreamEvent {
    /// First chunk with role and model info.
    MessageStart { message: MessageResponse },
    /// Text content delta.
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// Content block completed.
    ContentBlockStop { index: usize },
    /// Tool call start (OpenAI: delta.tool_calls with id + function.name).
    ToolCallStart { index: usize, id: String, name: String },
    /// Tool call argument delta (OpenAI: delta.tool_calls with function.arguments).
    ToolCallDelta { index: usize, arguments: String },
    /// Final usage stats.
    MessageDelta { delta: MessageDelta, usage: DeltaUsage },
    /// Stream ended.
    MessageStop,
    /// Keep-alive.
    Ping,
    /// Error from the API.
    Error { error: ApiError },
}

// Kept for backward compat — unused fields still referenced elsewhere.
pub type ContentBlock = serde_json::Value;

// ---------------------------------------------------------------------------
// ContentDelta
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
}

// Provide named constructors for the REPL code that matches on variants.
impl ContentDelta {
    pub fn text(&self) -> Option<&str> {
        match self {
            ContentDelta::TextDelta { text } => Some(text),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: Vec<serde_json::Value>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaUsage {
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountTokensResponse {
    #[serde(default)]
    pub input_tokens: u64,
}

// ---------------------------------------------------------------------------
// OpenAI SSE chunk types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    model: Option<String>,
    choices: Option<Vec<ChunkChoice>>,
    usage: Option<ChunkUsage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    #[allow(dead_code)]
    index: Option<usize>,
    delta: Option<ChunkDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    index: Option<usize>,
    id: Option<String>,
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// SSE parser
// ---------------------------------------------------------------------------

/// Parse an OpenAI-compatible SSE byte stream into typed events.
pub fn parse_sse_stream<S>(byte_stream: S) -> impl Stream<Item = Result<StreamEvent, ApiError>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    let line_stream = byte_stream
        .map(|result| result.map_err(|e| ApiError::Network {
            message: e.to_string(),
            source: e,
        }));

    // Use a VecDeque to buffer multiple events from a single SSE data line.
    stream::unfold(
        (Box::pin(line_stream), String::new(), std::collections::VecDeque::<StreamEvent>::new(), false),
        |(mut byte_stream, mut buffer, mut pending, mut done)| async move {
            if done && pending.is_empty() {
                return None;
            }

            // Drain pending events first.
            if let Some(evt) = pending.pop_front() {
                return Some((Ok(evt), (byte_stream, buffer, pending, done)));
            }

            if done {
                return None;
            }

            loop {
                // Try to extract a complete SSE event from the buffer.
                while let Some(event) = extract_sse_event(&mut buffer) {
                    let event = event.trim().to_string();
                    if event.is_empty() {
                        continue;
                    }

                    // Handle multi-line SSE events (some providers send event: + data: on separate lines)
                    let data_line = event.lines()
                        .find(|l| l.starts_with("data: "))
                        .or_else(|| if event.starts_with("data: ") { Some(event.as_str()) } else { None });

                    if let Some(line) = data_line {
                        let data = line.strip_prefix("data: ").unwrap_or(line).trim();
                        if data == "[DONE]" {
                            done = true;
                            return Some((Ok(StreamEvent::MessageStop), (byte_stream, buffer, pending, done)));
                        }
                        if let Some(mut events) = parse_chunk(data) {
                            if !events.is_empty() {
                                let first = events.remove(0);
                                for evt in events {
                                    pending.push_back(evt);
                                }
                                return Some((Ok(first), (byte_stream, buffer, pending, done)));
                            }
                        }
                    }
                }

                // Need more data from the stream.
                match byte_stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        done = true;
                        return Some((Err(e), (byte_stream, buffer, pending, done)));
                    }
                    None => {
                        done = true;
                        // Parse any remaining data in the buffer.
                        if !buffer.trim().is_empty() {
                            let remaining = buffer.trim().to_string();
                            buffer.clear();
                            for line in remaining.lines() {
                                let data = line.strip_prefix("data: ").unwrap_or(line).trim();
                                if data == "[DONE]" {
                                    return Some((Ok(StreamEvent::MessageStop), (byte_stream, buffer, pending, done)));
                                }
                                if let Some(mut events) = parse_chunk(data) {
                                    for evt in events.drain(..) {
                                        pending.push_back(evt);
                                    }
                                }
                            }
                            if let Some(evt) = pending.pop_front() {
                                return Some((Ok(evt), (byte_stream, buffer, pending, done)));
                            }
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Extract one SSE event (delimited by double newline) from the buffer.
fn extract_sse_event(buffer: &mut String) -> Option<String> {
    // SSE events are delimited by \n\n
    if let Some(pos) = buffer.find("\n\n") {
        let event = buffer[..pos].to_string();
        *buffer = buffer[pos + 2..].to_string();
        Some(event)
    } else if let Some(pos) = buffer.find("\r\n\r\n") {
        let event = buffer[..pos].to_string();
        *buffer = buffer[pos + 4..].to_string();
        Some(event)
    } else {
        None
    }
}

/// Parse an OpenAI chat completion chunk into StreamEvents.
fn parse_chunk(data: &str) -> Option<Vec<StreamEvent>> {
    let chunk: ChatCompletionChunk = serde_json::from_str(data).ok()?;
    let mut events = Vec::new();

    if let Some(choices) = chunk.choices {
        for choice in choices {
            let idx = choice.index.unwrap_or(0);

            if let Some(delta) = choice.delta {
                // Role-only delta = message start
                if delta.role.is_some() && delta.content.is_none() && delta.tool_calls.is_none() {
                    events.push(StreamEvent::MessageStart {
                        message: MessageResponse {
                            id: chunk.id.clone().unwrap_or_default(),
                            model: chunk.model.clone().unwrap_or_default(),
                            role: delta.role.unwrap_or_else(|| "assistant".into()),
                            content: vec![],
                            usage: None,
                        },
                    });
                }

                // Text content
                if let Some(text) = delta.content {
                    if !text.is_empty() {
                        events.push(StreamEvent::ContentBlockDelta {
                            index: idx,
                            delta: ContentDelta::TextDelta { text },
                        });
                    }
                }

                // Tool calls
                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        let tc_idx = tc.index.unwrap_or(0);
                        if let Some(func) = tc.function {
                            // If id + name present, this is the start of a new tool call
                            if let (Some(id), Some(name)) = (&tc.id, &func.name) {
                                events.push(StreamEvent::ToolCallStart {
                                    index: tc_idx,
                                    id: id.clone(),
                                    name: name.clone(),
                                });
                            }
                            // Argument chunks
                            if let Some(args) = func.arguments {
                                if !args.is_empty() {
                                    events.push(StreamEvent::ToolCallDelta {
                                        index: tc_idx,
                                        arguments: args,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Finish reason
            if let Some(reason) = choice.finish_reason {
                events.push(StreamEvent::MessageDelta {
                    delta: MessageDelta {
                        stop_reason: Some(reason),
                    },
                    usage: DeltaUsage { output_tokens: 0 },
                });
            }
        }
    }

    // Usage (some providers send it in the last chunk)
    if let Some(usage) = chunk.usage {
        events.push(StreamEvent::MessageDelta {
            delta: MessageDelta { stop_reason: None },
            usage: DeltaUsage {
                output_tokens: usage.completion_tokens.unwrap_or(0),
            },
        });
    }

    if events.is_empty() {
        None
    } else {
        Some(events)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_chunk() {
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let events = parse_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentBlockDelta { delta: ContentDelta::TextDelta { text }, .. } => {
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_parse_finish_chunk() {
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let events = parse_chunk(data).unwrap();
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageDelta { delta: MessageDelta { stop_reason: Some(r) }, .. } if r == "stop")));
    }

    #[test]
    fn test_parse_role_chunk() {
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        let events = parse_chunk(data).unwrap();
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStart { .. })));
    }

    #[test]
    fn test_extract_sse_event() {
        let mut buf = "data: {\"test\":1}\n\ndata: {\"test\":2}\n\n".to_string();
        let e1 = extract_sse_event(&mut buf).unwrap();
        assert_eq!(e1, "data: {\"test\":1}");
        let e2 = extract_sse_event(&mut buf).unwrap();
        assert_eq!(e2, "data: {\"test\":2}");
        assert!(extract_sse_event(&mut buf).is_none());
    }
}
