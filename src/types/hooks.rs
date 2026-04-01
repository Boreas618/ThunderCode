//! Hook types -- progress, results, and prompt elicitation.
//!
//! Ported from ref/types/hooks.ts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::messages::Message;
use crate::types::permissions::PermissionUpdate;

// ============================================================================
// HookEvent
// ============================================================================

/// All possible hook event names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    SessionStart,
    Setup,
    SubagentStart,
    PermissionDenied,
    Notification,
    PermissionRequest,
    Elicitation,
    ElicitationResult,
    CwdChanged,
    FileChanged,
    WorktreeCreate,
}

/// All valid hook event values.
pub const HOOK_EVENTS: &[HookEvent] = &[
    HookEvent::PreToolUse,
    HookEvent::PostToolUse,
    HookEvent::PostToolUseFailure,
    HookEvent::UserPromptSubmit,
    HookEvent::SessionStart,
    HookEvent::Setup,
    HookEvent::SubagentStart,
    HookEvent::PermissionDenied,
    HookEvent::Notification,
    HookEvent::PermissionRequest,
    HookEvent::Elicitation,
    HookEvent::ElicitationResult,
    HookEvent::CwdChanged,
    HookEvent::FileChanged,
    HookEvent::WorktreeCreate,
];

// ============================================================================
// HookProgress
// ============================================================================

/// Progress update from a hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookProgress {
    #[serde(rename = "type")]
    pub progress_type: HookProgressType,
    pub hook_event: HookEvent,
    pub hook_name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookProgressType {
    #[serde(rename = "hook_progress")]
    HookProgress,
}

// ============================================================================
// HookBlockingError
// ============================================================================

/// A blocking error from a hook that prevents continuation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookBlockingError {
    pub blocking_error: String,
    pub command: String,
}

// ============================================================================
// Prompt Elicitation
// ============================================================================

/// A prompt request from a hook to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    /// Request ID.
    pub prompt: String,
    pub message: String,
    pub options: Vec<PromptOption>,
}

/// An option in a prompt request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOption {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Response to a prompt request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    /// Request ID (mirrors `PromptRequest.prompt`).
    pub prompt_response: String,
    pub selected: String,
}

// ============================================================================
// PermissionRequestResult
// ============================================================================

/// Result of a PermissionRequest hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum PermissionRequestResult {
    #[serde(rename = "allow")]
    Allow {
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<HashMap<String, serde_json::Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    #[serde(rename = "deny")]
    Deny {
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}

// ============================================================================
// HookResult
// ============================================================================

/// Result of executing a single hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_error: Option<HookBlockingError>,
    pub outcome: HookOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prevent_continuation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_behavior: Option<HookPermissionBehavior>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_permission_decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_user_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_mcp_tool_output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_request_result: Option<PermissionRequestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<bool>,
}

/// Outcome of a hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookOutcome {
    Success,
    Blocking,
    NonBlockingError,
    Cancelled,
}

/// Permission behavior that a hook can specify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPermissionBehavior {
    Ask,
    Deny,
    Allow,
    Passthrough,
}

// ============================================================================
// AggregatedHookResult
// ============================================================================

/// Aggregated result from running multiple hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedHookResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_errors: Option<Vec<HookBlockingError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prevent_continuation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_permission_decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_behavior: Option<HookPermissionBehavior>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_contexts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_user_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_mcp_tool_output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_request_result: Option<PermissionRequestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<bool>,
}

// ============================================================================
// Sync / Async Hook JSON Output
// ============================================================================

/// JSON output from a sync hook response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHookResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#continue: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<HookDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

/// Hook decision (approve or block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    Approve,
    Block,
}

/// JSON output from an async hook response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncHookResponse {
    pub r#async: bool, // always true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub async_timeout: Option<f64>,
}

/// Combined hook JSON output (sync or async).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookJSONOutput {
    Async(AsyncHookResponse),
    Sync(SyncHookResponse),
}

impl HookJSONOutput {
    pub fn is_sync(&self) -> bool {
        matches!(self, HookJSONOutput::Sync(_))
    }

    pub fn is_async(&self) -> bool {
        matches!(self, HookJSONOutput::Async(_))
    }
}

// ============================================================================
// Hook-specific output variants
// ============================================================================

/// Hook-specific output, discriminated by `hookEventName`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    PreToolUse {
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_decision: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_decision_reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<HashMap<String, serde_json::Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    UserPromptSubmit {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    SessionStart {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        initial_user_message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
    },
    Setup {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    SubagentStart {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PostToolUse {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_mcp_tool_output: Option<serde_json::Value>,
    },
    PostToolUseFailure {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PermissionDenied {
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<bool>,
    },
    Notification {
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PermissionRequest {
        decision: PermissionRequestResult,
    },
    Elicitation {
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<ElicitationAction>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<HashMap<String, serde_json::Value>>,
    },
    ElicitationResult {
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<ElicitationAction>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<HashMap<String, serde_json::Value>>,
    },
    CwdChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
    },
    FileChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
    },
    WorktreeCreate {
        worktree_path: String,
    },
}

/// Elicitation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}
