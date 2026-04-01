//! Content block types for the message model.
//!
//! These represent the individual blocks within a message's content array,
//! covering text, images, tool use, tool results, and thinking blocks.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ContentBlock -- the main tagged union
// ---------------------------------------------------------------------------

/// A content block within a message. Tagged on `"type"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
    },

    #[serde(rename = "image")]
    Image {
        source: ImageSource,
    },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },

    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: String,
    },

    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
    },

    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "server_tool_result")]
    ServerToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
}

// ---------------------------------------------------------------------------
// Input-side variant (ContentBlockParam)
// ---------------------------------------------------------------------------

/// Input-side content block, used when constructing API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
    },

    #[serde(rename = "image")]
    Image {
        source: ImageSource,
    },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Content of a tool result -- either plain text or a list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Plain text result.
    Text(String),
    /// Structured result as a list of content blocks.
    Blocks(Vec<ContentBlock>),
}

/// Source data for an image content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    /// Always `"base64"`.
    #[serde(rename = "type")]
    pub source_type: String,
    /// MIME type, e.g. `"image/png"`, `"image/jpeg"`, `"image/gif"`, `"image/webp"`.
    pub media_type: String,
    /// Base64-encoded image data.
    pub data: String,
}

impl ImageSource {
    /// Create a new base64-encoded image source.
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            source_type: "base64".to_owned(),
            media_type: media_type.into(),
            data: data.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl ContentBlock {
    /// Extract text content if this is a `Text` block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Check if this is a `ToolUse` block.
    pub fn is_tool_use(&self) -> bool {
        matches!(self, ContentBlock::ToolUse { .. })
    }

    /// Check if this is a `ToolResult` block.
    pub fn is_tool_result(&self) -> bool {
        matches!(self, ContentBlock::ToolResult { .. })
    }

    /// Extract the tool use ID if this is a `ToolUse` block.
    pub fn tool_use_id(&self) -> Option<&str> {
        match self {
            ContentBlock::ToolUse { id, .. } => Some(id),
            _ => None,
        }
    }
}
