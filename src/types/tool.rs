//! Tool trait and supporting types.
//!
//! Ported from ref/Tool.ts -- the core tool abstraction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::ids::AgentId;
use crate::types::messages::Message;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};

// ============================================================================
// Tool Input Schema
// ============================================================================

/// JSON Schema for tool input. Must be `{"type": "object", ...}`.
pub type ToolInputJSONSchema = serde_json::Value;

// ============================================================================
// ValidationResult
// ============================================================================

/// Result of tool input validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValidationResult {
    Valid {
        result: ValidTag,
    },
    Invalid {
        result: InvalidTag,
        message: String,
        error_code: i32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ValidTag;

// Custom true/false literal serialization
impl serde::Serialize for InvalidTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(false)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InvalidTag;

impl<'de> serde::Deserialize<'de> for InvalidTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = bool::deserialize(deserializer)?;
        if !v {
            Ok(InvalidTag)
        } else {
            Err(serde::de::Error::custom("expected false"))
        }
    }
}

impl ValidationResult {
    /// Create a `Valid` result.
    pub fn valid() -> Self {
        ValidationResult::Valid { result: ValidTag }
    }

    /// Create an `Invalid` result.
    pub fn invalid(message: impl Into<String>, error_code: i32) -> Self {
        ValidationResult::Invalid {
            result: InvalidTag,
            message: message.into(),
            error_code,
        }
    }

    /// Check if the result is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid { .. })
    }
}

// ============================================================================
// InterruptBehavior
// ============================================================================

/// What happens when the user submits a new message while a tool is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterruptBehavior {
    /// Stop the tool and discard its result.
    Cancel,
    /// Keep running; the new message waits.
    Block,
}

// ============================================================================
// SearchReadInfo
// ============================================================================

/// Information about whether a tool use is a search/read operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchReadInfo {
    pub is_search: bool,
    pub is_read: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_list: Option<bool>,
}

// ============================================================================
// ToolCallResult
// ============================================================================

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_messages: Option<Vec<Message>>,
    /// MCP protocol metadata to pass through to SDK consumers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_meta: Option<McpMeta>,
}

/// MCP protocol metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMeta {
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Map<String, serde_json::Value>>,
}

/// MCP server/tool info for MCP-originating tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInfo {
    pub server_name: String,
    pub tool_name: String,
}

// ============================================================================
// ToolProgress types
// ============================================================================

/// Progress data for a tool execution, discriminated by `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolProgressData {
    #[serde(rename = "bash_progress")]
    Bash(BashProgress),

    #[serde(rename = "web_search_progress")]
    WebSearch(WebSearchProgress),

    #[serde(rename = "agent_progress")]
    Agent(AgentToolProgress),

    #[serde(rename = "mcp_progress")]
    Mcp(McpProgress),

    #[serde(rename = "skill_progress")]
    Skill(SkillToolProgress),

    #[serde(rename = "task_output_progress")]
    TaskOutput(TaskOutputProgress),

    #[serde(rename = "repl_progress")]
    Repl(ReplToolProgress),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchProgress {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<WebSearchResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentToolProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_progress: Option<Box<ToolProgressData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpProgress {
    pub server_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillToolProgress {
    pub skill_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputProgress {
    pub task_id: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplToolProgress {
    pub inner_tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_progress: Option<Box<ToolProgressData>>,
}

/// A progress event for a specific tool use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProgress {
    pub tool_use_id: String,
    pub data: ToolProgressData,
}

/// Combined progress type -- either tool progress or hook progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Progress {
    Tool(ToolProgressData),
    Hook(crate::types::hooks::HookProgress),
}

// ============================================================================
// CompactProgressEvent
// ============================================================================

/// Events emitted during compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CompactProgressEvent {
    #[serde(rename = "hooks_start")]
    HooksStart {
        hook_type: CompactHookType,
    },
    #[serde(rename = "compact_start")]
    CompactStart,
    #[serde(rename = "compact_end")]
    CompactEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactHookType {
    PreCompact,
    PostCompact,
    SessionStart,
}

// ============================================================================
// QueryChainTracking
// ============================================================================

/// Tracking data for chained queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryChainTracking {
    pub chain_id: String,
    pub depth: u32,
}

// ============================================================================
// ToolUseContext
// ============================================================================

/// The large context object passed to tool calls and permission checks.
///
/// In the TS codebase this carries many runtime callbacks; in Rust we keep
/// only the data fields that are representable without lifetimes/callbacks.
/// Runtime behavior (abort, state mutation, etc.) will be handled by separate
/// service traits in other crates.
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    pub options: ToolOptions,
    pub messages: Vec<Message>,
    #[allow(dead_code)]
    pub agent_id: Option<AgentId>,
    #[allow(dead_code)]
    pub agent_type: Option<String>,
    pub file_reading_limits: Option<FileReadingLimits>,
    pub glob_limits: Option<GlobLimits>,
    pub query_tracking: Option<QueryChainTracking>,
    pub tool_use_id: Option<String>,
    /// When true, preserve tool use results for viewable transcripts.
    pub preserve_tool_use_results: Option<bool>,
}

impl Default for ToolUseContext {
    fn default() -> Self {
        Self {
            options: ToolOptions::default(),
            messages: Vec::new(),
            agent_id: None,
            agent_type: None,
            file_reading_limits: None,
            glob_limits: None,
            query_tracking: None,
            tool_use_id: None,
            preserve_tool_use_results: None,
        }
    }
}

/// Options controlling tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOptions {
    /// Command names available in this session.
    pub commands: Vec<String>,
    pub debug: bool,
    pub main_loop_model: String,
    pub verbose: bool,
    pub is_non_interactive_session: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_budget_usd: Option<f64>,
    /// Custom system prompt that replaces the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_system_prompt: Option<String>,
    /// Additional system prompt appended after the main one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<String>,
    /// Override querySource for analytics tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_source: Option<String>,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            debug: false,
            main_loop_model: String::new(),
            verbose: false,
            is_non_interactive_session: false,
            max_budget_usd: None,
            custom_system_prompt: None,
            append_system_prompt: None,
            query_source: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadingLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
}

// ============================================================================
// ToolPermissionContext (re-exported from permissions, defined in Tool.ts)
// ============================================================================

// ToolPermissionContext is defined in permissions.rs and re-exported from there.

// ============================================================================
// Tool Trait
// ============================================================================

/// The core tool trait. Every tool implements this.
///
/// Tools are the primary way the LLM interacts with the system -- reading
/// files, running commands, searching, editing, etc.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Primary tool name (unique identifier).
    fn name(&self) -> &str;

    /// Optional aliases for backwards compatibility when a tool is renamed.
    fn aliases(&self) -> Vec<String> {
        vec![]
    }

    /// One-line capability phrase for keyword matching in ToolSearch.
    fn search_hint(&self) -> Option<&str> {
        None
    }

    /// Maximum result size in characters before persisting to disk.
    fn max_result_size_chars(&self) -> usize;

    /// Whether the tool is currently enabled.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Whether this tool use is read-only (doesn't modify the filesystem).
    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool performs irreversible operations.
    fn is_destructive(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool use can safely run concurrently with others.
    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// What happens when the user submits while this tool is running.
    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    /// When true, this tool is deferred and requires ToolSearch.
    fn should_defer(&self) -> bool {
        false
    }

    /// When true, this tool is never deferred.
    fn always_load(&self) -> bool {
        false
    }

    /// Whether this is an MCP-originating tool.
    fn is_mcp(&self) -> bool {
        false
    }

    /// Whether this is an LSP-originating tool.
    fn is_lsp(&self) -> bool {
        false
    }

    /// The Zod/JSON Schema for this tool's input.
    fn input_schema(&self) -> ToolInputJSONSchema;

    /// Optional direct JSON Schema (for MCP tools).
    fn input_json_schema(&self) -> Option<&ToolInputJSONSchema> {
        None
    }

    /// MCP server/tool info if this is an MCP tool.
    fn mcp_info(&self) -> Option<&McpInfo> {
        None
    }

    /// Execute the tool.
    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError>;

    /// Tool-specific permission check. Called after `validate_input`.
    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> PermissionResult {
        // Default: defer to general permission system.
        PermissionResult::allow(Some(input.clone()))
    }

    /// Validate the tool input before execution.
    async fn validate_input(
        &self,
        _input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        ValidationResult::valid()
    }

    /// Human-readable description of what this tool use will do.
    fn description(
        &self,
        input: &serde_json::Value,
        tool_permission_context: &ToolPermissionContext,
    ) -> String;

    /// System prompt contribution from this tool.
    async fn prompt(&self) -> String;

    /// User-facing name for display (may differ from `name()`).
    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        self.name().to_owned()
    }

    /// Present-tense activity description for spinner display.
    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Short summary for compact views.
    fn get_tool_use_summary(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Compact representation for the auto-mode security classifier.
    fn to_auto_classifier_input(&self, _input: &serde_json::Value) -> serde_json::Value {
        serde_json::Value::String(String::new())
    }

    /// Extract a file path from the input, if applicable.
    fn get_path(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    /// Whether this tool use is a search/read/list operation.
    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchReadInfo {
        SearchReadInfo::default()
    }
}

// ============================================================================
// ToolError
// ============================================================================

/// Errors that can occur during tool execution.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum ToolError {
    #[error("Tool execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Tool input validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Tool not found: {name}")]
    NotFound { name: String },

    #[error("Tool is disabled: {name}")]
    Disabled { name: String },

    #[error("Tool execution was cancelled")]
    Cancelled,

    #[error("Tool execution timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("{0}")]
    Other(String),
}

// ============================================================================
// Helper functions
// ============================================================================

/// Check if a tool matches the given name (primary name or alias).
pub fn tool_matches_name(tool: &dyn Tool, name: &str) -> bool {
    tool.name() == name || tool.aliases().iter().any(|a| a == name)
}

/// Find a tool by name or alias from a list of tools.
pub fn find_tool_by_name<'a>(tools: &'a [Box<dyn Tool>], name: &str) -> Option<&'a dyn Tool> {
    tools
        .iter()
        .find(|t| tool_matches_name(t.as_ref(), name))
        .map(|t| t.as_ref())
}

/// A collection of tools.
pub type Tools = Vec<Box<dyn Tool>>;
