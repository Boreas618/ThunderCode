//! Session history -- listing, loading, saving, and deleting sessions.
//!
//! Ported from ref/history.ts and the session-listing parts of
//! ref/utils/sessionStorage.ts.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use crate::types::messages::Message;

use crate::session::storage::{now_unix, SessionMetadata, SessionStorage};

// ---------------------------------------------------------------------------
// SessionEntry
// ---------------------------------------------------------------------------

/// Summary of a single session, used for listing.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub session_id: String,
    pub name: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub model: String,
    pub cwd: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// List recent sessions, sorted newest-first.
///
/// Scans `~/.thundercode/sessions/` for session directories, loads their metadata,
/// and returns at most `limit` entries ordered by `updated_at` descending.
/// Sessions without valid metadata are silently skipped.
pub fn list_sessions(limit: usize) -> Result<Vec<SessionEntry>> {
    let sessions_root = sessions_root_dir();
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&sessions_root)
        .with_context(|| format!("failed to read sessions dir: {:?}", sessions_root))?;

    let mut sessions: Vec<SessionEntry> = Vec::new();

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

        // Load metadata; skip sessions with missing/corrupt metadata.
        let metadata = match storage.load_metadata() {
            Ok(Some(m)) => m,
            _ => continue,
        };

        // Count messages without fully parsing them (just count lines).
        let message_count = count_lines(&storage.session_dir.join("messages.jsonl"));

        sessions.push(SessionEntry {
            session_id,
            name: metadata.name,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            message_count,
            model: metadata.model,
            cwd: metadata.cwd,
        });
    }

    // Sort newest-first by updated_at.
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions.truncate(limit);
    Ok(sessions)
}

/// Load all messages for a given session.
pub fn load_session(session_id: &str) -> Result<Vec<Message>> {
    let storage = SessionStorage::new(session_id);
    storage.load_messages()
}

/// Save a full set of messages for a given session (append each one).
///
/// This also writes/updates the session metadata with the current timestamp.
/// Existing messages already on disk are preserved because `append_message`
/// opens the file in append mode.
pub fn save_session(session_id: &str, messages: &[Message]) -> Result<()> {
    let storage = SessionStorage::new(session_id);

    for msg in messages {
        storage.append_message(msg)?;
    }

    // Update metadata timestamp.
    let mut metadata = storage
        .load_metadata()?
        .unwrap_or_else(|| SessionMetadata {
            session_id: session_id.to_string(),
            name: None,
            created_at: now_unix(),
            updated_at: now_unix(),
            model: String::new(),
            pr_number: None,
            cwd: String::new(),
        });

    metadata.updated_at = now_unix();
    storage.save_metadata(&metadata)?;

    Ok(())
}

/// Delete an entire session (removes the directory and all files).
pub fn delete_session(session_id: &str) -> Result<()> {
    let dir = SessionStorage::session_dir(session_id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("failed to delete session dir: {:?}", dir))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Root directory for all sessions: `~/.thundercode/sessions/`.
fn sessions_root_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".thundercode").join("sessions")
}

/// Count non-empty lines in a file. Returns 0 if the file does not exist or
/// cannot be read.
fn count_lines(path: &PathBuf) -> usize {
    use std::io::{BufRead, BufReader};
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .count()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::storage::SessionStorage;
    use crate::types::content::ContentBlock;
    use crate::types::messages::UserMessage;
    use uuid::Uuid;

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

    /// Creates a session in a temp dir and returns the temp dir guard plus
    /// the session ID.
    fn setup_temp_session(
        id: &str,
    ) -> (tempfile::TempDir, SessionStorage) {
        let tmp = tempfile::tempdir().unwrap();
        let storage = SessionStorage {
            session_dir: tmp.path().join(id),
        };
        (tmp, storage)
    }

    #[test]
    fn save_then_load_session() {
        let (_tmp, storage) = setup_temp_session("hist-1");

        let meta = SessionMetadata {
            session_id: "hist-1".into(),
            name: Some("test session".into()),
            created_at: 1700000000,
            updated_at: 1700000000,
            model: "primary-3-opus".into(),
            pr_number: None,
            cwd: "/tmp".into(),
        };
        storage.save_metadata(&meta).unwrap();
        storage.append_message(&make_user_msg("hello")).unwrap();
        storage.append_message(&make_user_msg("world")).unwrap();

        let loaded = storage.load_messages().unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn count_lines_helper() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        std::fs::write(&path, "{\"a\":1}\n{\"b\":2}\n\n{\"c\":3}\n").unwrap();
        assert_eq!(count_lines(&path), 3);
    }

    #[test]
    fn count_lines_missing_file() {
        let path = PathBuf::from("/nonexistent/file.jsonl");
        assert_eq!(count_lines(&path), 0);
    }
}
