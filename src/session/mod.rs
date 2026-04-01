//! ThunderCode session persistence and history.
//!
//! Provides JSONL-based session storage, history listing, conversation
//! recovery from incomplete sessions, and session restore.
//!
//! Ported from:
//! - ref/utils/sessionStorage.ts (storage, JSONL read/write)
//! - ref/history.ts (prompt history)
//! - ref/utils/conversationRecovery.ts (recovery/repair)

pub mod history;
pub mod recovery;
pub mod restore;
pub mod storage;

pub use history::{list_sessions, load_session, save_session, delete_session, SessionEntry};
pub use recovery::{detect_incomplete_sessions, recover_session, repair_message_chain};
pub use restore::{restore_session, RestoredSession};
pub use storage::{SessionStorage, SessionMetadata};
