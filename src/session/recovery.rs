//! Conversation recovery -- detecting and repairing incomplete sessions.
//!
//! Ported from ref/utils/conversationRecovery.ts.
//!
//! The TS code filters unresolved tool uses, orphaned thinking-only messages,
//! and whitespace-only assistant messages. It also detects mid-turn
//! interruptions. This module provides a simplified Rust equivalent focused
//! on the core repair logic.

use std::fs;

use anyhow::{Context, Result};
use crate::types::messages::Message;

use crate::session::storage::SessionStorage;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan all sessions and return IDs of those that appear incomplete.
///
/// A session is considered incomplete when its JSONL transcript ends with a
/// user message (the assistant never responded) or when metadata is present
/// but the message file is missing or empty.
pub fn detect_incomplete_sessions() -> Result<Vec<String>> {
    let sessions_root = sessions_root_dir();
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&sessions_root)
        .with_context(|| format!("failed to read sessions dir: {:?}", sessions_root))?;

    let mut incomplete = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let session_id = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let storage = SessionStorage { session_dir: path };

        // A session is incomplete if:
        //  1. Metadata exists but messages file is missing or empty.
        //  2. The last message is a user message (assistant never replied).
        let has_metadata = storage.load_metadata().ok().flatten().is_some();
        let messages = storage.load_messages().unwrap_or_default();

        if has_metadata && messages.is_empty() {
            incomplete.push(session_id);
            continue;
        }

        if let Some(last) = messages.last() {
            if last.is_user() {
                incomplete.push(session_id);
            }
        }
    }

    Ok(incomplete)
}

/// Load messages from an incomplete session, applying repairs.
///
/// This combines `load_messages` with `repair_message_chain` to return a
/// cleaned-up conversation suitable for resumption.
pub fn recover_session(session_id: &str) -> Result<Vec<Message>> {
    let storage = SessionStorage::new(session_id);
    let mut messages = storage.load_messages()?;
    repair_message_chain(&mut messages);
    Ok(messages)
}

/// Repair a message chain in place.
///
/// This applies the same class of fixes as the TS `deserializeMessages`:
///
/// 1. **Filter orphaned assistant messages** -- assistant messages that
///    contain only empty or whitespace text are removed.
/// 2. **Filter trailing unresolved tool uses** -- if the last assistant
///    message contains a tool-use block without a matching tool-result from
///    the user, the incomplete tail is trimmed.
/// 3. **Ensure alternation** -- consecutive messages of the same role are
///    collapsed (the later one wins) so the conversation alternates between
///    user and assistant.
pub fn repair_message_chain(messages: &mut Vec<Message>) {
    // Pass 1: remove whitespace-only assistant messages.
    messages.retain(|msg| {
        if let Message::Assistant(asst) = msg {
            // Keep the message if it has at least one non-empty text block
            // or any non-text block (tool_use, etc.).
            let has_content = asst.content.iter().any(|block| {
                match block {
                    crate::types::content::ContentBlock::Text { text, .. } => {
                        !text.trim().is_empty()
                    }
                    _ => true,
                }
            });
            return has_content;
        }
        true
    });

    // Pass 2: trim trailing unresolved tool uses.
    //
    // If the conversation ends with an assistant message whose last content
    // block is a tool_use, and there is no following user message with a
    // matching tool_result, we remove the trailing assistant message.
    trim_trailing_unresolved_tool_use(messages);

    // Pass 3: ensure user/assistant alternation.
    //
    // Walk the chain and, when two consecutive messages have the same role,
    // keep only the second one. System/progress/other types are left in
    // place.
    dedup_consecutive_same_role(messages);
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Root directory for all sessions: `~/.thundercode/sessions/`.
fn sessions_root_dir() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".thundercode").join("sessions")
}

/// Remove a trailing assistant message that ends with an unresolved tool_use.
fn trim_trailing_unresolved_tool_use(messages: &mut Vec<Message>) {
    if messages.is_empty() {
        return;
    }

    // Check if the last message is an assistant with a trailing tool_use.
    let needs_trim = if let Some(Message::Assistant(asst)) = messages.last() {
        asst.content.last().map_or(false, |block| {
            matches!(block, crate::types::content::ContentBlock::ToolUse { .. })
        })
    } else {
        false
    };

    if needs_trim {
        messages.pop();
    }
}

/// Remove consecutive messages of the same role, keeping the later one.
///
/// Only considers User and Assistant messages; other types are passed
/// through without deduplication.
fn dedup_consecutive_same_role(messages: &mut Vec<Message>) {
    if messages.len() < 2 {
        return;
    }

    let mut i = 0;
    while i + 1 < messages.len() {
        let same_role = match (&messages[i], &messages[i + 1]) {
            (Message::User(_), Message::User(_)) => true,
            (Message::Assistant(_), Message::Assistant(_)) => true,
            _ => false,
        };
        if same_role {
            messages.remove(i);
        } else {
            i += 1;
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::content::ContentBlock;
    use crate::types::messages::{AssistantMessage, UserMessage};
    use uuid::Uuid;

    fn user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: text.into(),
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

    fn assistant_msg(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: text.into(),
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

    fn whitespace_assistant() -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "   \n\n  ".into(),
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

    fn assistant_with_tool_use() -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![ContentBlock::ToolUse {
                id: "tool-1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "ls"}),
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

    #[test]
    fn repair_removes_whitespace_only_assistant() {
        let mut msgs = vec![
            user_msg("hello"),
            whitespace_assistant(),
            assistant_msg("real response"),
        ];
        repair_message_chain(&mut msgs);
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].is_user());
        assert!(msgs[1].is_assistant());
    }

    #[test]
    fn repair_trims_trailing_unresolved_tool_use() {
        let mut msgs = vec![
            user_msg("do something"),
            assistant_with_tool_use(),
        ];
        repair_message_chain(&mut msgs);
        // The trailing assistant with unresolved tool_use should be removed.
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].is_user());
    }

    #[test]
    fn repair_dedup_consecutive_same_role() {
        let mut msgs = vec![
            user_msg("first"),
            user_msg("second"),
            assistant_msg("reply"),
        ];
        repair_message_chain(&mut msgs);
        assert_eq!(msgs.len(), 2);
        // The first user msg should be removed, keeping "second".
        assert!(msgs[0].is_user());
        assert!(msgs[1].is_assistant());
    }

    #[test]
    fn repair_noop_on_valid_chain() {
        let mut msgs = vec![
            user_msg("hello"),
            assistant_msg("hi"),
            user_msg("bye"),
            assistant_msg("goodbye"),
        ];
        repair_message_chain(&mut msgs);
        assert_eq!(msgs.len(), 4);
    }
}
