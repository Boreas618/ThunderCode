//! Protocol types for the bridge environments API.
//!
//! Ported from ref/bridge/types.ts`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default per-session timeout (24 hours).
pub const DEFAULT_SESSION_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Reusable login guidance appended to bridge auth errors.
pub const BRIDGE_LOGIN_INSTRUCTION: &str =
    "Remote Control is only available with subscriptions. \
     Please use `/login` to sign in with your account.";

/// Full error printed when bridge mode is run without auth.
pub const BRIDGE_LOGIN_ERROR: &str =
    "Error: You must be logged in to use Remote Control.\n\n\
     Remote Control is only available with subscriptions. \
     Please use `/login` to sign in with your account.";

/// Shown when the user disconnects Remote Control.
pub const REMOTE_CONTROL_DISCONNECTED_MSG: &str = "Remote Control disconnected.";

/// State of the bridge connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeState {
    /// Bridge is initialized and ready to connect.
    Ready,
    /// Bridge is connected and polling for work.
    Connected,
    /// Bridge lost connection and is attempting to reconnect.
    Reconnecting,
    /// Bridge encountered a fatal error and has stopped.
    Failed,
}

/// How the bridge chooses session working directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpawnMode {
    /// One session in cwd, bridge tears down when it ends.
    SingleSession,
    /// Persistent server, every session gets an isolated git worktree.
    Worktree,
    /// Persistent server, every session shares cwd.
    SameDir,
}

/// Worker type values for environment registration metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeWorkerType {
    ThunderCode,
    ThunderCodeAssistant,
}

impl std::fmt::Display for BridgeWorkerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeWorkerType::ThunderCode => write!(f, "thundercode"),
            BridgeWorkerType::ThunderCodeAssistant => write!(f, "thundercode_assistant"),
        }
    }
}

/// Configuration for the bridge.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Working directory for sessions.
    pub dir: String,
    /// Machine hostname for display.
    pub machine_name: String,
    /// Git branch name.
    pub branch: String,
    /// Git repo URL (if available).
    pub git_repo_url: Option<String>,
    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,
    /// How sessions get their working directories.
    pub spawn_mode: SpawnMode,
    /// Enable verbose logging.
    pub verbose: bool,
    /// Enable sandbox mode for child processes.
    pub sandbox: bool,
    /// Client-generated UUID identifying this bridge instance.
    pub bridge_id: String,
    /// Worker type metadata for web client filtering.
    pub worker_type: String,
    /// Client-generated UUID for idempotent environment registration.
    pub environment_id: String,
    /// Backend-issued environment ID to reuse on re-register (for resume).
    pub reuse_environment_id: Option<String>,
    /// API base URL the bridge is connected to.
    pub api_base_url: String,
    /// Session ingress base URL for WebSocket connections.
    pub session_ingress_url: String,
    /// Debug file path for logging.
    pub debug_file: Option<String>,
    /// Per-session timeout in milliseconds.
    pub session_timeout_ms: Option<u64>,
}

/// Data payload within a work response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkData {
    /// Work type: `"session"` or `"healthcheck"`.
    #[serde(rename = "type")]
    pub work_type: String,
    /// Session ID or healthcheck ID.
    pub id: String,
}

/// Response from the `pollForWork` API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResponse {
    /// Unique work item ID.
    pub id: String,
    /// Always `"work"`.
    #[serde(rename = "type")]
    pub work_type: String,
    /// Environment ID this work belongs to.
    pub environment_id: String,
    /// Work item state.
    pub state: String,
    /// Payload containing session/healthcheck info.
    pub data: WorkData,
    /// Base64url-encoded JSON with session tokens and configuration.
    pub secret: String,
    /// When the work item was created.
    pub created_at: String,
}

/// Decoded work secret containing session credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSecret {
    /// Secret version.
    pub version: i32,
    /// JWT for session ingress authentication.
    pub session_ingress_token: String,
    /// API base URL for the session.
    pub api_base_url: String,
    /// Whether to use CCR v2 transport.
    #[serde(default)]
    pub use_code_sessions: bool,
}

/// Status of a completed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDoneStatus {
    /// Session completed successfully.
    Completed,
    /// Session failed with an error.
    Failed,
    /// Session was interrupted (SIGTERM/SIGINT).
    Interrupted,
}

/// Type of session activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionActivityType {
    ToolStart,
    Text,
    Result,
    Error,
}

/// A recorded activity from a running session.
#[derive(Debug, Clone)]
pub struct SessionActivity {
    /// Activity type.
    pub activity_type: SessionActivityType,
    /// Human-readable summary (e.g. "Editing src/foo.rs").
    pub summary: String,
    /// When this activity occurred (millis since epoch).
    pub timestamp: u64,
}

/// A permission response event sent back to a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponseEvent {
    /// Always `"control_response"`.
    #[serde(rename = "type")]
    pub event_type: String,
    /// The response payload.
    pub response: PermissionResponseInner,
}

/// Inner payload of a permission response event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponseInner {
    /// Always `"success"`.
    pub subtype: String,
    /// The request ID this responds to.
    pub request_id: String,
    /// The permission decision payload.
    pub response: serde_json::Value,
}

/// Options for spawning a session child process.
#[derive(Debug, Clone)]
pub struct SessionSpawnOpts {
    /// Session ID.
    pub session_id: String,
    /// SDK URL for the child process.
    pub sdk_url: String,
    /// Access token for session ingress.
    pub access_token: String,
    /// Whether to use CCR v2 transport.
    pub use_ccr_v2: bool,
    /// Worker epoch (required when `use_ccr_v2` is true).
    pub worker_epoch: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_state() {
        assert_eq!(BridgeState::Ready, BridgeState::Ready);
        assert_ne!(BridgeState::Ready, BridgeState::Connected);
    }

    #[test]
    fn test_spawn_mode_serde() {
        let json = serde_json::to_string(&SpawnMode::SingleSession).unwrap();
        assert_eq!(json, "\"single-session\"");

        let mode: SpawnMode = serde_json::from_str("\"worktree\"").unwrap();
        assert_eq!(mode, SpawnMode::Worktree);

        let mode: SpawnMode = serde_json::from_str("\"same-dir\"").unwrap();
        assert_eq!(mode, SpawnMode::SameDir);
    }

    #[test]
    fn test_work_response_deserialize() {
        let json = r#"{
            "id": "work-123",
            "type": "work",
            "environment_id": "env-456",
            "state": "pending",
            "data": {"type": "session", "id": "sess-789"},
            "secret": "base64secret",
            "created_at": "2025-01-01T00:00:00Z"
        }"#;
        let response: WorkResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "work-123");
        assert_eq!(response.data.work_type, "session");
        assert_eq!(response.data.id, "sess-789");
    }

    #[test]
    fn test_worker_type_display() {
        assert_eq!(BridgeWorkerType::ThunderCode.to_string(), "thundercode");
        assert_eq!(
            BridgeWorkerType::ThunderCodeAssistant.to_string(),
            "thundercode_assistant"
        );
    }
}
