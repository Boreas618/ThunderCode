//! Session restore -- load a complete session from storage for resumption.
//!
//! Combines storage, metadata, and recovery into a single entry point.

use anyhow::{Context, Result};
use crate::types::messages::Message;

use crate::session::recovery::repair_message_chain;
use crate::session::storage::{SessionMetadata, SessionStorage};

// ---------------------------------------------------------------------------
// RestoredSession
// ---------------------------------------------------------------------------

/// A fully loaded and repaired session, ready for resumption.
#[derive(Debug, Clone)]
pub struct RestoredSession {
    /// The conversation messages, repaired for API validity.
    pub messages: Vec<Message>,
    /// Session metadata.
    pub metadata: SessionMetadata,
    /// The model used in the session (convenience copy from metadata).
    pub model: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Restore a session by ID.
///
/// 1. Loads session metadata.
/// 2. Loads the JSONL transcript.
/// 3. Runs `repair_message_chain` to clean up any corruption or incomplete
///    tool-use chains (matches the TS `deserializeMessages` behaviour).
/// 4. Returns a `RestoredSession` ready for the REPL.
pub fn restore_session(session_id: &str) -> Result<RestoredSession> {
    let storage = SessionStorage::new(session_id);

    let metadata = storage
        .load_metadata()?
        .with_context(|| format!("no metadata found for session {}", session_id))?;

    let mut messages = storage
        .load_messages()
        .with_context(|| format!("failed to load messages for session {}", session_id))?;

    // Apply the same recovery passes as the TS deserializeMessages.
    repair_message_chain(&mut messages);

    let model = metadata.model.clone();

    Ok(RestoredSession {
        messages,
        metadata,
        model,
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::storage::{now_unix, SessionMetadata, SessionStorage};
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

    #[test]
    fn restore_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = SessionStorage {
            session_dir: tmp.path().join("restore-test"),
        };

        let meta = SessionMetadata {
            session_id: "restore-test".into(),
            name: Some("test".into()),
            created_at: now_unix(),
            updated_at: now_unix(),
            model: "primary-3-opus".into(),
            pr_number: None,
            cwd: "/tmp".into(),
        };
        storage.save_metadata(&meta).unwrap();
        storage.append_message(&user_msg("hello")).unwrap();
        storage.append_message(&assistant_msg("world")).unwrap();

        // Restore by directly using storage path.
        let mut messages = storage.load_messages().unwrap();
        repair_message_chain(&mut messages);
        let loaded_meta = storage.load_metadata().unwrap().unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(loaded_meta.model, "primary-3-opus");
    }

    #[test]
    fn restore_repairs_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = SessionStorage {
            session_dir: tmp.path().join("repair-test"),
        };

        let meta = SessionMetadata {
            session_id: "repair-test".into(),
            name: None,
            created_at: now_unix(),
            updated_at: now_unix(),
            model: "primary-3-haiku".into(),
            pr_number: None,
            cwd: "/tmp".into(),
        };
        storage.save_metadata(&meta).unwrap();

        // Write a conversation that ends with a trailing tool use (incomplete).
        storage.append_message(&user_msg("do something")).unwrap();
        storage
            .append_message(&Message::Assistant(AssistantMessage {
                role: "assistant".into(),
                content: vec![ContentBlock::ToolUse {
                    id: "tu-1".into(),
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
            }))
            .unwrap();

        let mut messages = storage.load_messages().unwrap();
        repair_message_chain(&mut messages);

        // The unresolved tool_use assistant should have been trimmed.
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_user());
    }
}
