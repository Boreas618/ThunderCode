//! Request types for OpenAI-compatible chat completions API.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CreateMessageRequest (OpenAI format)
// ---------------------------------------------------------------------------

/// Top-level request to `/v1/chat/completions`.
#[derive(Debug, Clone, Serialize)]
pub struct CreateMessageRequest {
    pub model: String,

    pub messages: Vec<ApiMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl CreateMessageRequest {
    /// Create a basic request with model, max_tokens, and messages.
    pub fn new(model: &str, max_tokens: u32, messages: Vec<ApiMessage>) -> Self {
        Self {
            model: model.to_owned(),
            messages,
            max_tokens: Some(max_tokens),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            tool_choice: None,
        }
    }

    /// Set stream = true and return self (builder pattern).
    pub fn with_streaming(mut self) -> Self {
        self.stream = true;
        self
    }
}

// ---------------------------------------------------------------------------
// CountTokensRequest (heuristic, not all providers support this)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CountTokensRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    #[serde(skip)]
    pub betas: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// ApiMessage (OpenAI format)
// ---------------------------------------------------------------------------

/// A message in OpenAI chat format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: ApiContent,

    /// Tool call ID (for role=tool messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,

    /// Tool calls made by the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Function name (for role=tool messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiContent {
    Text(String),
    Blocks(Vec<ContentBlockParam>),
}

impl ApiMessage {
    /// Create a user message with plain text.
    pub fn user(text: &str) -> Self {
        Self {
            role: "user".to_owned(),
            content: ApiContent::Text(text.to_owned()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// Create a system message.
    pub fn system(text: &str) -> Self {
        Self {
            role: "system".to_owned(),
            content: ApiContent::Text(text.to_owned()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(text: &str) -> Self {
        Self {
            role: "assistant".to_owned(),
            content: ApiContent::Text(text.to_owned()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".to_owned(),
            content: ApiContent::Text(content.to_owned()),
            tool_call_id: Some(tool_call_id.to_owned()),
            tool_calls: None,
            name: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ContentBlockParam
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
    },

    #[serde(rename = "image_url")]
    ImageUrl {
        image_url: ImageUrl,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Tool definitions (OpenAI format)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String), // "auto", "none", "required"
    Specific {
        #[serde(rename = "type")]
        tool_type: String,
        function: ToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Tool calls (OpenAI response format)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

// ---------------------------------------------------------------------------
// SystemBlock (kept for backward compat in prompt building)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SystemBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<serde_json::Value>,
    },
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}
