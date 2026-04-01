//! Session memory compaction.
//!
//! Ported from ref/services/compact/sessionMemoryCompact.ts`. When session
//! memory is available (a running summary of the conversation extracted by a
//! background agent), we can use it as a drop-in replacement for the model-
//! generated compact summary, skipping the expensive summarisation API call.
//!
//! The flow:
//! 1. Check whether session memory content exists and is non-empty.
//! 2. Determine which messages have already been summarised (via
//!    `last_summarized_message_id`).
//! 3. Calculate how many recent messages to keep (expand backwards until
//!    minimum token and text-block-count thresholds are met).
//! 4. Build a `CompactionResult` using the session memory as the summary.

use crate::types::Message;

use super::prompt::get_compact_user_summary_message;
use super::summarize::{estimate_message_tokens, CompactionResult};

// ---------------------------------------------------------------------------
// SessionMemoryCompactConfig
// ---------------------------------------------------------------------------

/// Configuration thresholds for session memory compaction.
#[derive(Debug, Clone)]
pub struct SessionMemoryCompactConfig {
    /// Minimum tokens to preserve after compaction.
    pub min_tokens: u64,

    /// Minimum number of messages with text blocks to keep.
    pub min_text_block_messages: usize,

    /// Maximum tokens to preserve after compaction (hard cap).
    pub max_tokens: u64,
}

impl Default for SessionMemoryCompactConfig {
    fn default() -> Self {
        Self {
            min_tokens: 10_000,
            min_text_block_messages: 5,
            max_tokens: 40_000,
        }
    }
}

// ---------------------------------------------------------------------------
// session_memory_compact
// ---------------------------------------------------------------------------

/// Attempt session-memory-based compaction.
///
/// Returns `Some(CompactionResult)` when session memory is available and
/// non-empty; `None` if the caller should fall back to model-based compaction.
///
/// # Arguments
///
/// * `messages` -- The full conversation.
/// * `session_memory` -- The session memory content (or `None` if unavailable).
/// * `last_summarized_message_id` -- UUID string of the last message that has
///   been incorporated into session memory, or `None` for resumed sessions.
/// * `config` -- Thresholds controlling how many messages to keep.
/// * `transcript_path` -- Optional path to the full transcript file.
pub fn session_memory_compact(
    messages: &[Message],
    session_memory: Option<&str>,
    last_summarized_message_id: Option<&str>,
    config: &SessionMemoryCompactConfig,
    transcript_path: Option<&str>,
) -> Option<CompactionResult> {
    let memory = session_memory?;
    if memory.trim().is_empty() {
        return None;
    }

    // Determine the boundary between summarised and unsummarised messages.
    let last_summarized_index = if let Some(id) = last_summarized_message_id {
        let idx = messages
            .iter()
            .position(|m| m.uuid().map(|u| u.to_string()) == Some(id.to_owned()));
        match idx {
            Some(i) => i as isize,
            // The ID was not found -- we cannot determine the boundary.
            None => return None,
        }
    } else {
        // Resumed session: no boundary known, so start from the end.
        (messages.len() as isize) - 1
    };

    // Calculate the starting index for messages to keep.
    let start_index =
        calculate_messages_to_keep_index(messages, last_summarized_index as usize, config);

    let messages_to_keep = &messages[start_index..];
    let removed_count = start_index;
    let preserved_count = messages_to_keep.len();

    // Build the summary message using session memory content.
    let summary = get_compact_user_summary_message(
        memory,
        true,  // suppress follow-up questions
        transcript_path,
        true,  // recent messages preserved
    );

    let estimated_pre = estimate_message_tokens(messages);
    let estimated_post = estimate_message_tokens(messages_to_keep)
        + rough_token_estimate(&summary);

    Some(CompactionResult {
        summary,
        preserved_count,
        removed_count,
        tokens_saved: estimated_pre.saturating_sub(estimated_post),
        pre_compact_token_count: Some(estimated_pre),
        post_compact_token_count: estimated_post,
    })
}

// ---------------------------------------------------------------------------
// Message-keep calculation
// ---------------------------------------------------------------------------

/// Calculate the starting index for messages to keep after compaction.
///
/// Starts from `last_summarized_index + 1`, then expands backwards to meet
/// the minimum token and text-block-message thresholds. Stops expanding if
/// the `max_tokens` cap is hit.
fn calculate_messages_to_keep_index(
    messages: &[Message],
    last_summarized_index: usize,
    config: &SessionMemoryCompactConfig,
) -> usize {
    if messages.is_empty() {
        return 0;
    }

    let mut start_index = if last_summarized_index < messages.len() {
        last_summarized_index + 1
    } else {
        messages.len()
    };

    // Calculate current tokens and text-block count from start_index to end.
    let mut total_tokens: u64 = 0;
    let mut text_block_count: usize = 0;

    for msg in &messages[start_index..] {
        total_tokens += estimate_message_tokens(std::slice::from_ref(msg));
        if has_text_blocks(msg) {
            text_block_count += 1;
        }
    }

    // Already at the max cap?
    if total_tokens >= config.max_tokens {
        return start_index;
    }

    // Already meet both minimums?
    if total_tokens >= config.min_tokens && text_block_count >= config.min_text_block_messages {
        return start_index;
    }

    // Expand backwards until both minimums are met or max cap is reached.
    while start_index > 0 {
        start_index -= 1;
        let msg = &messages[start_index];
        total_tokens += estimate_message_tokens(std::slice::from_ref(msg));
        if has_text_blocks(msg) {
            text_block_count += 1;
        }

        if total_tokens >= config.max_tokens {
            break;
        }
        if total_tokens >= config.min_tokens && text_block_count >= config.min_text_block_messages {
            break;
        }
    }

    start_index
}

/// Check whether a message contains text blocks.
fn has_text_blocks(message: &Message) -> bool {
    match message {
        Message::User(u) => u.content.iter().any(|b| {
            matches!(b, crate::types::ContentBlock::Text { text } if !text.is_empty())
        }),
        Message::Assistant(a) => a.content.iter().any(|b| {
            matches!(b, crate::types::ContentBlock::Text { .. })
        }),
        _ => false,
    }
}

/// Rough token estimate (~4 chars per token).
fn rough_token_estimate(text: &str) -> u64 {
    (text.len() as f64 / 4.0).ceil() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, UserMessage};
    use uuid::Uuid;

    fn text_user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            role: "user".to_owned(),
            content: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
            uuid: Uuid::new_v4(),
            is_bash_input: None,
            is_paste: None,
            is_queued: None,
            command_name: None,
            origin: None,
            is_meta: None,
        })
    }

    #[test]
    fn returns_none_without_memory() {
        let messages = vec![text_user_msg("hello")];
        let result = session_memory_compact(&messages, None, None, &Default::default(), None);
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_with_empty_memory() {
        let messages = vec![text_user_msg("hello")];
        let result =
            session_memory_compact(&messages, Some("  "), None, &Default::default(), None);
        assert!(result.is_none());
    }

    #[test]
    fn produces_result_with_valid_memory() {
        let messages: Vec<Message> = (0..10).map(|i| text_user_msg(&format!("msg {i}"))).collect();
        let config = SessionMemoryCompactConfig {
            min_tokens: 0,
            min_text_block_messages: 0,
            max_tokens: 100,
        };
        let result = session_memory_compact(
            &messages,
            Some("Session memory summary here."),
            None,
            &config,
            None,
        );
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.summary.contains("Session memory summary here."));
        assert!(r.preserved_count + r.removed_count == messages.len());
    }
}
