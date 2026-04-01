//! Micro-compaction: lightweight in-place content trimming.
//!
//! Ported from ref/services/compact/microCompact.ts`. Unlike full compaction
//! (which calls the model to summarise), micro-compaction replaces old tool
//! result content with a short placeholder. This is much cheaper and can run
//! before every API call without an extra model query.

use crate::types::{ContentBlock, Message};

/// The placeholder message that replaces cleared tool results.
pub const CLEARED_MESSAGE: &str = "[Old tool result content cleared]";

/// Tool names whose results are eligible for micro-compaction.
const COMPACTABLE_TOOLS: &[&str] = &[
    "Read",
    "Bash",
    "Grep",
    "Glob",
    "Edit",
    "Write",
    "WebSearch",
    "WebFetch",
];

// ---------------------------------------------------------------------------
// MicrocompactResult
// ---------------------------------------------------------------------------

/// Result of a micro-compaction pass.
#[derive(Debug)]
pub struct MicrocompactResult {
    /// The (possibly modified) messages.
    pub messages: Vec<Message>,

    /// Number of tool results that were cleared.
    pub cleared_count: usize,

    /// Estimated tokens freed by clearing tool results.
    pub tokens_saved: u64,
}

// ---------------------------------------------------------------------------
// micro_compact
// ---------------------------------------------------------------------------

/// Run micro-compaction on a message list.
///
/// Walks the messages and replaces old compactable tool result content with
/// `CLEARED_MESSAGE`, keeping only the `keep_recent` most recent results
/// intact.
///
/// Returns the modified messages along with stats about what was cleared.
pub fn micro_compact(messages: &[Message], keep_recent: usize) -> MicrocompactResult {
    // First pass: collect all compactable tool_use IDs in encounter order.
    let compactable_ids = collect_compactable_tool_ids(messages);

    if compactable_ids.is_empty() || compactable_ids.len() <= keep_recent {
        return MicrocompactResult {
            messages: messages.to_vec(),
            cleared_count: 0,
            tokens_saved: 0,
        };
    }

    // Keep the most recent `keep_recent` IDs; clear the rest.
    let keep_start = compactable_ids.len().saturating_sub(keep_recent.max(1));
    let keep_set: std::collections::HashSet<&str> = compactable_ids[keep_start..]
        .iter()
        .map(String::as_str)
        .collect();
    let clear_set: std::collections::HashSet<&str> = compactable_ids[..keep_start]
        .iter()
        .map(String::as_str)
        .collect();

    let mut tokens_saved: u64 = 0;
    let mut cleared_count: usize = 0;

    let result: Vec<Message> = messages
        .iter()
        .map(|msg| {
            if let Message::User(u) = msg {
                let mut touched = false;
                let new_content: Vec<ContentBlock> = u
                    .content
                    .iter()
                    .map(|block| {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = block
                        {
                            if clear_set.contains(tool_use_id.as_str())
                                && !is_already_cleared(content)
                            {
                                tokens_saved += estimate_tool_result_tokens(content);
                                cleared_count += 1;
                                touched = true;
                                return ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: crate::types::ToolResultContent::Text(
                                        CLEARED_MESSAGE.to_owned(),
                                    ),
                                    is_error: *is_error,
                                };
                            }
                        }
                        block.clone()
                    })
                    .collect();

                if touched {
                    let mut u_clone = u.clone();
                    u_clone.content = new_content;
                    return Message::User(u_clone);
                }
            }
            msg.clone()
        })
        .collect();

    // Suppress the result if nothing was actually freed.
    let _ = &keep_set; // keep borrow alive until here

    MicrocompactResult {
        messages: result,
        cleared_count,
        tokens_saved,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Collect tool_use IDs from assistant messages whose tool name is compactable.
fn collect_compactable_tool_ids(messages: &[Message]) -> Vec<String> {
    let compactable: std::collections::HashSet<&str> = COMPACTABLE_TOOLS.iter().copied().collect();
    let mut ids = Vec::new();

    for msg in messages {
        if let Message::Assistant(a) = msg {
            for block in &a.content {
                if let ContentBlock::ToolUse { id, name, .. } = block {
                    if compactable.contains(name.as_str()) {
                        ids.push(id.clone());
                    }
                }
            }
        }
    }

    ids
}

/// Check whether a tool result has already been cleared.
fn is_already_cleared(content: &crate::types::ToolResultContent) -> bool {
    match content {
        crate::types::ToolResultContent::Text(t) => t == CLEARED_MESSAGE,
        crate::types::ToolResultContent::Blocks(_) => false,
    }
}

/// Rough token estimate for a tool result's content.
fn estimate_tool_result_tokens(content: &crate::types::ToolResultContent) -> u64 {
    match content {
        crate::types::ToolResultContent::Text(t) => {
            (t.len() as f64 / 4.0).ceil() as u64
        }
        crate::types::ToolResultContent::Blocks(blocks) => {
            let mut total = 0u64;
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    total += (text.len() as f64 / 4.0).ceil() as u64;
                }
            }
            total
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessage, UserMessage};
    use uuid::Uuid;

    fn make_tool_use_msg(tool_id: &str, tool_name: &str) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".to_owned(),
            content: vec![ContentBlock::ToolUse {
                id: tool_id.to_owned(),
                name: tool_name.to_owned(),
                input: serde_json::json!({}),
            }],
            uuid: Uuid::new_v4(),
            api_error: None,
            model: None,
            stop_reason: None,
            usage: None,
            cost_usd: None,
            duration_ms: None,
        })
    }

    fn make_tool_result_msg(tool_id: &str, result_text: &str) -> Message {
        Message::User(UserMessage {
            role: "user".to_owned(),
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_id.to_owned(),
                content: crate::types::ToolResultContent::Text(result_text.to_owned()),
                is_error: None,
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
    fn micro_compact_clears_old_results() {
        let messages = vec![
            make_tool_use_msg("t1", "Read"),
            make_tool_result_msg("t1", "file contents..."),
            make_tool_use_msg("t2", "Bash"),
            make_tool_result_msg("t2", "command output..."),
            make_tool_use_msg("t3", "Read"),
            make_tool_result_msg("t3", "recent file contents..."),
        ];

        let result = micro_compact(&messages, 1);
        assert_eq!(result.cleared_count, 2);
        assert!(result.tokens_saved > 0);

        // The most recent (t3) should be preserved.
        if let Message::User(u) = &result.messages[5] {
            if let ContentBlock::ToolResult { content, .. } = &u.content[0] {
                match content {
                    crate::types::ToolResultContent::Text(t) => {
                        assert_eq!(t, "recent file contents...");
                    }
                    _ => panic!("expected text content"),
                }
            }
        }

        // Older results should be cleared.
        if let Message::User(u) = &result.messages[1] {
            if let ContentBlock::ToolResult { content, .. } = &u.content[0] {
                match content {
                    crate::types::ToolResultContent::Text(t) => {
                        assert_eq!(t, CLEARED_MESSAGE);
                    }
                    _ => panic!("expected text content"),
                }
            }
        }
    }

    #[test]
    fn micro_compact_noop_when_few_results() {
        let messages = vec![
            make_tool_use_msg("t1", "Read"),
            make_tool_result_msg("t1", "contents"),
        ];

        let result = micro_compact(&messages, 5);
        assert_eq!(result.cleared_count, 0);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn non_compactable_tools_are_skipped() {
        let messages = vec![
            make_tool_use_msg("t1", "CustomTool"),
            make_tool_result_msg("t1", "custom output"),
        ];

        let result = micro_compact(&messages, 0);
        assert_eq!(result.cleared_count, 0);
    }
}
