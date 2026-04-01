//! Message filtering, transformation, and echo-dedup machinery for bridge
//! transport.
//!
//! Ported from ref/bridge/bridgeMessaging.ts`.

use std::collections::HashSet;

/// FIFO-bounded set backed by a circular buffer. Evicts the oldest entry
/// when capacity is reached, keeping memory usage constant at O(capacity).
///
/// Used for echo-dedup: messages we sent recently are tracked so we can
/// ignore them when the server echoes them back.
pub struct BoundedUuidSet {
    capacity: usize,
    ring: Vec<Option<String>>,
    set: HashSet<String>,
    write_idx: usize,
}

impl BoundedUuidSet {
    /// Create a new bounded UUID set with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            ring: vec![None; capacity],
            set: HashSet::with_capacity(capacity),
            write_idx: 0,
        }
    }

    /// Add a UUID to the set. If at capacity, evicts the oldest entry.
    pub fn add(&mut self, uuid: String) {
        if self.set.contains(&uuid) {
            return;
        }

        // Evict the entry at the current write position (if occupied).
        if let Some(ref evicted) = self.ring[self.write_idx] {
            self.set.remove(evicted);
        }

        self.ring[self.write_idx] = Some(uuid.clone());
        self.set.insert(uuid);
        self.write_idx = (self.write_idx + 1) % self.capacity;
    }

    /// Check if a UUID is in the set.
    pub fn has(&self, uuid: &str) -> bool {
        self.set.contains(uuid)
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.set.clear();
        self.ring.fill(None);
        self.write_idx = 0;
    }

    /// Current number of entries in the set.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

/// Type of an SDK message, determined by the `type` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkMessageType {
    User,
    Assistant,
    Result,
    ControlRequest,
    ControlResponse,
    ControlCancelRequest,
    Other(String),
}

impl SdkMessageType {
    /// Parse a message type string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "result" => Self::Result,
            "control_request" => Self::ControlRequest,
            "control_response" => Self::ControlResponse,
            "control_cancel_request" => Self::ControlCancelRequest,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Check if a JSON value looks like a valid SDK message (has a string `type` field).
pub fn is_sdk_message(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        .is_some()
}

/// Check if a message is a control response.
pub fn is_control_response(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        == Some("control_response")
}

/// Check if a message is a control request.
pub fn is_control_request(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        == Some("control_request")
}

/// Extract the UUID from an SDK message, if present.
pub fn extract_uuid(value: &serde_json::Value) -> Option<&str> {
    value
        .as_object()
        .and_then(|obj| obj.get("uuid"))
        .and_then(|u| u.as_str())
}

/// Extract the message type from an SDK message.
pub fn extract_message_type(value: &serde_json::Value) -> Option<SdkMessageType> {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        .map(SdkMessageType::from_str)
}

/// Map tool names to human-readable verbs for status display.
pub fn tool_verb(tool_name: &str) -> &'static str {
    match tool_name {
        "Read" | "FileReadTool" => "Reading",
        "Write" | "FileWriteTool" => "Writing",
        "Edit" | "MultiEdit" | "FileEditTool" => "Editing",
        "Bash" | "BashTool" => "Running",
        "Glob" | "Grep" | "GlobTool" | "GrepTool" => "Searching",
        "WebFetch" => "Fetching",
        "WebSearch" => "Searching",
        "Task" => "Running task",
        "NotebookEditTool" => "Editing notebook",
        "LSP" => "LSP",
        _ => "Using",
    }
}

/// Build a brief summary of a tool invocation for display.
pub fn tool_summary(name: &str, input: &serde_json::Value) -> String {
    let verb = tool_verb(name);
    let target = input
        .get("file_path")
        .or_else(|| input.get("filePath"))
        .or_else(|| input.get("pattern"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            input
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(60).collect())
        })
        .or_else(|| {
            input
                .get("url")
                .or_else(|| input.get("query"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    if target.is_empty() {
        verb.to_string()
    } else {
        format!("{verb} {target}")
    }
}

/// Sanitize a session ID for use in file names.
pub fn safe_filename_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_uuid_set_basic() {
        let mut set = BoundedUuidSet::new(3);
        assert!(set.is_empty());

        set.add("a".to_string());
        set.add("b".to_string());
        set.add("c".to_string());
        assert_eq!(set.len(), 3);
        assert!(set.has("a"));
        assert!(set.has("b"));
        assert!(set.has("c"));
    }

    #[test]
    fn test_bounded_uuid_set_eviction() {
        let mut set = BoundedUuidSet::new(3);
        set.add("a".to_string());
        set.add("b".to_string());
        set.add("c".to_string());

        // Adding a 4th should evict "a".
        set.add("d".to_string());
        assert!(!set.has("a"));
        assert!(set.has("b"));
        assert!(set.has("c"));
        assert!(set.has("d"));
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_bounded_uuid_set_dedup() {
        let mut set = BoundedUuidSet::new(3);
        set.add("a".to_string());
        set.add("a".to_string());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_bounded_uuid_set_clear() {
        let mut set = BoundedUuidSet::new(3);
        set.add("a".to_string());
        set.add("b".to_string());
        set.clear();
        assert!(set.is_empty());
        assert!(!set.has("a"));
    }

    #[test]
    fn test_is_sdk_message() {
        assert!(is_sdk_message(&serde_json::json!({"type": "assistant"})));
        assert!(!is_sdk_message(&serde_json::json!({"content": "no type"})));
        assert!(!is_sdk_message(&serde_json::json!(null)));
    }

    #[test]
    fn test_tool_verb() {
        assert_eq!(tool_verb("Read"), "Reading");
        assert_eq!(tool_verb("Bash"), "Running");
        assert_eq!(tool_verb("Grep"), "Searching");
        assert_eq!(tool_verb("UnknownTool"), "Using");
    }

    #[test]
    fn test_tool_summary() {
        let input = serde_json::json!({"file_path": "src/main.rs"});
        assert_eq!(tool_summary("Read", &input), "Reading src/main.rs");

        let input = serde_json::json!({"command": "cargo build"});
        assert_eq!(tool_summary("Bash", &input), "Running cargo build");

        let input = serde_json::json!({});
        assert_eq!(tool_summary("Read", &input), "Reading");
    }

    #[test]
    fn test_safe_filename_id() {
        assert_eq!(safe_filename_id("session-123"), "session-123");
        assert_eq!(safe_filename_id("../bad/path"), "___bad_path");
        assert_eq!(safe_filename_id("ok_id"), "ok_id");
    }

    #[test]
    fn test_extract_uuid() {
        let msg = serde_json::json!({"type": "user", "uuid": "abc-123"});
        assert_eq!(extract_uuid(&msg), Some("abc-123"));

        let msg = serde_json::json!({"type": "user"});
        assert_eq!(extract_uuid(&msg), None);
    }
}
