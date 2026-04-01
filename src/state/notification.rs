//! Notification types for transient UI messages.
//!
//! Ported from ref context/notifications.ts -- simplified to the core
//! priority + message + timeout model.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// NotificationPriority
// ---------------------------------------------------------------------------

/// How urgently a notification should be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationPriority {
    Low,
    Medium,
    High,
    /// Bypass the queue and display immediately.
    Immediate,
}

// ---------------------------------------------------------------------------
// Notification
// ---------------------------------------------------------------------------

/// A transient notification displayed in the TUI footer/status area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier.
    pub id: String,
    /// Human-readable message.
    pub message: String,
    /// Display priority -- higher-priority notifications preempt lower ones.
    pub priority: NotificationPriority,
    /// When the notification was created (epoch ms).
    pub created_at: u64,
    /// If `Some`, auto-dismiss after this many milliseconds.
    pub timeout_ms: Option<u64>,
}

impl Notification {
    /// Create a new notification with a generated UUID.
    pub fn new(
        message: impl Into<String>,
        priority: NotificationPriority,
        timeout_ms: Option<u64>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            message: message.into(),
            priority,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            timeout_ms,
        }
    }

    /// Convenience: low-priority notification with a 3-second timeout.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, NotificationPriority::Low, Some(3_000))
    }

    /// Convenience: medium-priority notification with a 5-second timeout.
    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(message, NotificationPriority::Medium, Some(5_000))
    }

    /// Convenience: high-priority notification with no timeout.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, NotificationPriority::High, None)
    }

    /// Convenience: immediate notification with no timeout.
    pub fn immediate(message: impl Into<String>) -> Self {
        Self::new(message, NotificationPriority::Immediate, None)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_has_low_priority_and_timeout() {
        let n = Notification::info("hello");
        assert_eq!(n.priority, NotificationPriority::Low);
        assert_eq!(n.timeout_ms, Some(3_000));
        assert_eq!(n.message, "hello");
        assert!(!n.id.is_empty());
    }

    #[test]
    fn error_has_high_priority_no_timeout() {
        let n = Notification::error("boom");
        assert_eq!(n.priority, NotificationPriority::High);
        assert_eq!(n.timeout_ms, None);
    }

    #[test]
    fn serde_roundtrip() {
        let n = Notification::warn("caution");
        let json = serde_json::to_string(&n).unwrap();
        let parsed: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(n, parsed);
    }
}
