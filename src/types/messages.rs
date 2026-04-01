//! Message model -- the core conversation data types.
//!
//! The types/message.ts file is not in the ref copy, but the message model is
//! inferred from imports across Tool.ts, logs.ts, and other reference files.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::content::ContentBlock;
use crate::types::tool::ToolProgressData;

// ---------------------------------------------------------------------------
// Message -- top-level tagged union
// ---------------------------------------------------------------------------

/// The core message type -- a tagged union of all message variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "user")]
    User(UserMessage),

    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),

    #[serde(rename = "system")]
    System(SystemMessage),

    #[serde(rename = "progress")]
    Progress(ProgressMessage),

    #[serde(rename = "tool_use_summary")]
    ToolUseSummary(ToolUseSummaryMessage),

    #[serde(rename = "tombstone")]
    Tombstone(TombstoneMessage),

    #[serde(rename = "attachment")]
    Attachment(AttachmentMessage),
}

// ---------------------------------------------------------------------------
// UserMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// Always `"user"`.
    pub role: String,
    /// Content blocks.
    pub content: Vec<ContentBlock>,
    /// Unique message identifier.
    pub uuid: Uuid,

    // -- metadata --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_bash_input: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_paste: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_queued: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<MessageOrigin>,
    /// When true, the message is meta (model-visible but hidden from the user).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
}

// ---------------------------------------------------------------------------
// AssistantMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// Always `"assistant"`.
    pub role: String,
    /// Content blocks.
    pub content: Vec<ContentBlock>,
    /// Unique message identifier.
    pub uuid: Uuid,

    // -- API response metadata --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_error: Option<ApiError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// SystemMessage
// ---------------------------------------------------------------------------

/// System messages with many subtypes, discriminated by `system_type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "system_type")]
pub enum SystemMessage {
    /// Informational message displayed to the user.
    #[serde(rename = "informational")]
    Informational {
        content: String,
        level: SystemMessageLevel,
    },

    /// Result of a local command (e.g. `/help`).
    #[serde(rename = "local_command")]
    LocalCommand(SystemLocalCommandMessage),

    /// API error surfaced to the user.
    #[serde(rename = "api_error")]
    ApiError {
        content: String,
        error_type: String,
    },

    /// Boundary marker for compact mode.
    #[serde(rename = "compact_boundary")]
    CompactBoundary {
        uuid: Uuid,
        summary: Option<String>,
    },

    /// Bridge status message.
    #[serde(rename = "bridge_status")]
    BridgeStatus {
        status: String,
        message: Option<String>,
    },

    /// Warning about a permission or security issue.
    #[serde(rename = "warning")]
    Warning { content: String },

    /// Session start marker.
    #[serde(rename = "session_start")]
    SessionStart { session_id: String },

    /// Session end marker.
    #[serde(rename = "session_end")]
    SessionEnd { session_id: String },
}

// ---------------------------------------------------------------------------
// ProgressMessage
// ---------------------------------------------------------------------------

/// Progress update for an in-flight tool use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub tool_use_id: String,
    pub data: ToolProgressData,
}

// ---------------------------------------------------------------------------
// ToolUseSummaryMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseSummaryMessage {
    pub summaries: Vec<ToolUseSummary>,
}

/// Summary of a single tool use, shown in compact views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseSummary {
    pub tool_use_id: String,
    pub tool_name: String,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// TombstoneMessage
// ---------------------------------------------------------------------------

/// Placeholder for a removed message, preserving the UUID for ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TombstoneMessage {
    pub uuid: Uuid,
}

// ---------------------------------------------------------------------------
// AttachmentMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMessage {
    pub memory_files: Vec<MemoryFile>,
}

/// A memory file (e.g. RULES.md) attached to the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFile {
    pub path: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Origin of a user message (SDK, CLI, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOrigin {
    #[serde(rename = "type")]
    pub origin_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// API error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

/// Token usage for an API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
}

/// System message severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemMessageLevel {
    Info,
    Warn,
    Error,
    Debug,
}

/// Local command result message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLocalCommandMessage {
    pub command_name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl Message {
    /// Get the UUID of this message, if it has one.
    pub fn uuid(&self) -> Option<Uuid> {
        match self {
            Message::User(m) => Some(m.uuid),
            Message::Assistant(m) => Some(m.uuid),
            Message::Tombstone(m) => Some(m.uuid),
            _ => None,
        }
    }

    /// Check if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self, Message::User(_))
    }

    /// Check if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self, Message::Assistant(_))
    }

    /// Check if this is a system message.
    pub fn is_system(&self) -> bool {
        matches!(self, Message::System(_))
    }
}
