//! JSONL session storage.
//!
//! Each session is stored as a JSONL file (one JSON object per line) under
//! `~/.thundercode/sessions/{session_id}/`. Messages are appended incrementally
//! to avoid rewriting the entire file on each update.
//!
//! Ported from ref/utils/sessionStorage.ts.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use crate::types::messages::Message;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SessionMetadata
// ---------------------------------------------------------------------------

/// Metadata about a stored session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub name: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<String>,
    pub cwd: String,
}

// ---------------------------------------------------------------------------
// SessionStorage
// ---------------------------------------------------------------------------

/// JSONL-based storage for a single session.
///
/// Messages are stored in `messages.jsonl` inside the session directory.
/// Metadata is stored in `metadata.json` as a single JSON object.
pub struct SessionStorage {
    pub session_dir: PathBuf,
}

impl SessionStorage {
    /// Create a new `SessionStorage` for the given session ID.
    ///
    /// The session directory is created lazily on the first write.
    pub fn new(session_id: &str) -> Self {
        Self {
            session_dir: Self::session_dir(session_id),
        }
    }

    /// Return the session directory path for a given session ID.
    ///
    /// Layout: `~/.thundercode/sessions/{session_id}/`
    pub fn session_dir(session_id: &str) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".thundercode").join("sessions").join(session_id)
    }

    // -- private helpers ----------------------------------------------------

    fn messages_path(&self) -> PathBuf {
        self.session_dir.join("messages.jsonl")
    }

    fn metadata_path(&self) -> PathBuf {
        self.session_dir.join("metadata.json")
    }

    /// Ensure the session directory exists.
    fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.session_dir)
            .with_context(|| format!("failed to create session dir: {:?}", self.session_dir))?;
        Ok(())
    }

    // -- message persistence ------------------------------------------------

    /// Append a single message to the JSONL transcript.
    ///
    /// The message is serialised as a single JSON line and appended to the
    /// file. This avoids rewriting the entire transcript on every turn.
    pub fn append_message(&self, message: &Message) -> Result<()> {
        self.ensure_dir()?;
        let path = self.messages_path();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("failed to open messages file: {:?}", path))?;

        let line = serde_json::to_string(message)
            .context("failed to serialize message")?;
        writeln!(file, "{}", line)
            .with_context(|| format!("failed to append to messages file: {:?}", path))?;

        Ok(())
    }

    /// Load all messages from the JSONL transcript.
    ///
    /// Corrupt or unparseable lines are silently skipped so that a partially
    /// written file does not prevent loading the rest of the conversation.
    pub fn load_messages(&self) -> Result<Vec<Message>> {
        let path = self.messages_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&path)
            .with_context(|| format!("failed to open messages file: {:?}", path))?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => continue, // skip unreadable lines
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Message>(trimmed) {
                Ok(msg) => messages.push(msg),
                Err(_) => {
                    // Skip corrupt/malformed lines -- matches TS behaviour of
                    // silently skipping parse failures in JSONL readers.
                    continue;
                }
            }
        }

        Ok(messages)
    }

    // -- metadata persistence -----------------------------------------------

    /// Write session metadata (overwrites previous).
    pub fn save_metadata(&self, metadata: &SessionMetadata) -> Result<()> {
        self.ensure_dir()?;
        let path = self.metadata_path();
        let json = serde_json::to_string_pretty(metadata)
            .context("failed to serialize session metadata")?;
        fs::write(&path, json)
            .with_context(|| format!("failed to write metadata: {:?}", path))?;
        Ok(())
    }

    /// Load session metadata, if it exists.
    ///
    /// Returns `Ok(None)` when the metadata file does not exist.
    pub fn load_metadata(&self) -> Result<Option<SessionMetadata>> {
        let path = self.metadata_path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read metadata: {:?}", path))?;
        let metadata: SessionMetadata = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse metadata: {:?}", path))?;
        Ok(Some(metadata))
    }
}

/// Helper: return the current Unix timestamp in seconds.
pub fn now_unix() -> u64 {
    Utc::now().timestamp() as u64
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::messages::{UserMessage, AssistantMessage};
    use crate::types::content::ContentBlock;
    use uuid::Uuid;
    use std::io::Write;

    /// Helper to create a temp session storage rooted in a temp dir.
    fn temp_storage() -> (tempfile::TempDir, SessionStorage) {
        let tmp = tempfile::tempdir().unwrap();
        let storage = SessionStorage {
            session_dir: tmp.path().join("test-session"),
        };
        (tmp, storage)
    }

    fn make_user_msg(text: &str) -> Message {
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

    fn make_assistant_msg(text: &str) -> Message {
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
    fn append_and_load_messages() {
        let (_tmp, storage) = temp_storage();

        let m1 = make_user_msg("hello");
        let m2 = make_assistant_msg("hi there");

        storage.append_message(&m1).unwrap();
        storage.append_message(&m2).unwrap();

        let loaded = storage.load_messages().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].is_user());
        assert!(loaded[1].is_assistant());
    }

    #[test]
    fn load_empty_returns_empty_vec() {
        let (_tmp, storage) = temp_storage();
        let loaded = storage.load_messages().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let (_tmp, storage) = temp_storage();

        // Write a valid message, then garbage, then another valid message.
        storage.append_message(&make_user_msg("first")).unwrap();

        let path = storage.messages_path();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "{{not valid json!!!").unwrap();
        writeln!(file, "").unwrap(); // blank line

        storage.append_message(&make_user_msg("third")).unwrap();

        let loaded = storage.load_messages().unwrap();
        assert_eq!(loaded.len(), 2, "corrupt/blank lines should be skipped");
    }

    #[test]
    fn metadata_round_trip() {
        let (_tmp, storage) = temp_storage();

        let meta = SessionMetadata {
            session_id: "test-123".into(),
            name: Some("my session".into()),
            created_at: 1700000000,
            updated_at: 1700000100,
            model: "primary-3-opus".into(),
            pr_number: None,
            cwd: "/tmp/project".into(),
        };

        storage.save_metadata(&meta).unwrap();
        let loaded = storage.load_metadata().unwrap().unwrap();
        assert_eq!(loaded.session_id, "test-123");
        assert_eq!(loaded.name.as_deref(), Some("my session"));
        assert_eq!(loaded.model, "primary-3-opus");
    }

    #[test]
    fn load_metadata_missing_returns_none() {
        let (_tmp, storage) = temp_storage();
        let loaded = storage.load_metadata().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn incremental_append_does_not_rewrite() {
        let (_tmp, storage) = temp_storage();

        storage.append_message(&make_user_msg("a")).unwrap();
        let size_after_one = fs::metadata(storage.messages_path()).unwrap().len();

        storage.append_message(&make_user_msg("b")).unwrap();
        let size_after_two = fs::metadata(storage.messages_path()).unwrap().len();

        // File grew -- it was appended to, not rewritten from scratch.
        assert!(size_after_two > size_after_one);
    }
}
