//! Protocol types for remote session communication.
//!
//! These types mirror the SDK message protocol used between the client and CCR
//! (ThunderCode Runner) sessions over WebSocket.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A message from the SDK (assistant output, tool results, progress, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMessage {
    /// Message type discriminant (e.g. "user", "assistant", "result").
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Optional UUID for echo-dedup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    /// The message payload (type-specific, flattened).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A control request from the server (permission prompt, interrupt, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlRequest {
    /// Always `"control_request"`.
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Unique request ID for matching responses.
    pub request_id: String,

    /// The inner request payload.
    pub request: ControlRequestInner,
}

/// Inner payload of a control request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequestInner {
    /// Request subtype: `"can_use_tool"`, `"interrupt"`, `"initialize"`, etc.
    pub subtype: String,

    /// Tool name (present when `subtype == "can_use_tool"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Tool input (present when `subtype == "can_use_tool"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<HashMap<String, serde_json::Value>>,

    /// Tool use ID (present when `subtype == "can_use_tool"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Model name (present when `subtype == "set_model"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Permission mode (present when `subtype == "set_permission_mode"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Max thinking tokens (present when `subtype == "set_max_thinking_tokens"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_thinking_tokens: Option<i64>,
}

/// A control response sent back to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    /// Always `"control_response"`.
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Inner response payload.
    pub response: ControlResponseInner,
}

/// Inner payload of a control response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponseInner {
    /// Response subtype: `"success"` or `"error"`.
    pub subtype: String,

    /// The request ID this response is for.
    pub request_id: String,

    /// Error message (present when `subtype == "error"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Success response payload (present when `subtype == "success"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
}

/// A cancel request from the server (cancelling a pending permission prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlCancelRequest {
    /// Always `"control_cancel_request"`.
    #[serde(rename = "type")]
    pub msg_type: String,

    /// The request ID being cancelled.
    pub request_id: String,
}

/// Union of all message types that can arrive on the sessions WebSocket.
///
/// Note: this enum is primarily used for documentation. In practice, incoming
/// messages are parsed as `serde_json::Value` and dispatched based on the
/// `type` field, because the SDK message type is a broad discriminated union.
#[derive(Debug, Clone)]
pub enum SessionsMessage {
    /// Control request from server.
    ControlRequest(SdkControlRequest),

    /// Control response (acknowledgment).
    ControlResponse(ControlResponse),

    /// Cancel a pending control request.
    ControlCancelRequest(SdkControlCancelRequest),

    /// Standard SDK message (assistant, user, result, etc.).
    Sdk(SdkMessage),
}

/// Permission response for remote sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum RemotePermissionResponse {
    /// Allow the tool use, optionally with modified input.
    #[serde(rename = "allow")]
    Allow {
        updated_input: HashMap<String, serde_json::Value>,
    },

    /// Deny the tool use with a reason.
    #[serde(rename = "deny")]
    Deny { message: String },
}

/// Events emitted by the remote session to the local consumer.
#[derive(Debug, Clone)]
pub enum RemoteEvent {
    /// An SDK message was received.
    Message(SdkMessage),

    /// A permission request needs user approval.
    PermissionRequest {
        request: ControlRequestInner,
        request_id: String,
    },

    /// A pending permission request was cancelled by the server.
    PermissionCancelled {
        request_id: String,
        tool_use_id: Option<String>,
    },

    /// WebSocket connection established.
    Connected,

    /// WebSocket connection lost permanently.
    Disconnected,

    /// Transient connection loss, reconnect scheduled.
    Reconnecting,

    /// An error occurred.
    Error(String),
}

/// Content that can be sent as a remote message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMessageContent {
    /// The text content of the message.
    pub text: String,

    /// Optional attachments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<serde_json::Value>>,
}

/// Check if a parsed JSON value looks like a valid sessions message.
pub fn is_sessions_message(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_response_serde() {
        let allow = RemotePermissionResponse::Allow {
            updated_input: HashMap::new(),
        };
        let json = serde_json::to_string(&allow).unwrap();
        assert!(json.contains("allow"));

        let deny = RemotePermissionResponse::Deny {
            message: "not allowed".to_string(),
        };
        let json = serde_json::to_string(&deny).unwrap();
        assert!(json.contains("deny"));
        assert!(json.contains("not allowed"));
    }

    #[test]
    fn test_is_sessions_message() {
        let valid = serde_json::json!({"type": "assistant", "content": "hello"});
        assert!(is_sessions_message(&valid));

        let invalid = serde_json::json!({"content": "no type field"});
        assert!(!is_sessions_message(&invalid));

        let null = serde_json::json!(null);
        assert!(!is_sessions_message(&null));
    }

    #[test]
    fn test_sdk_message_serde() {
        let msg = SdkMessage {
            msg_type: "assistant".to_string(),
            uuid: Some("abc-123".to_string()),
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("abc-123"));
    }
}
