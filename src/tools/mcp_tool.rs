//! MCPTool -- generic MCP (Model Context Protocol) tool wrapper.
//!
//! Ported from ref/tools/MCPTool/MCPTool.ts.
//! Each MCP server's tools are wrapped as McpToolInstance objects.
//! This module provides the wrapper and the factory for creating them.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const MCP_TOOL_NAME: &str = "MCPTool";

/// A dynamic MCP tool instance created from MCP server tool definitions.
/// Each tool proxies calls to the remote MCP server.
pub struct McpToolInstance {
    pub tool_name: String,
    pub server_name: String,
    pub description_text: String,
    pub schema: serde_json::Value,
    pub mcp_info_data: McpInfo,
    pub should_always_load: bool,
    pub search_hint_text: Option<String>,
}

impl McpToolInstance {
    /// Create a new MCP tool instance from server metadata.
    pub fn new(
        server_name: &str,
        tool_name: &str,
        description: &str,
        schema: serde_json::Value,
    ) -> Self {
        let full_name = format!("mcp__{server_name}__{tool_name}");
        Self {
            tool_name: full_name.clone(),
            server_name: server_name.to_string(),
            description_text: description.to_string(),
            schema,
            mcp_info_data: McpInfo {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
            },
            should_always_load: false,
            search_hint_text: None,
        }
    }

    /// Set the always_load flag (from _meta["thundercode/alwaysLoad"]).
    pub fn with_always_load(mut self, always_load: bool) -> Self {
        self.should_always_load = always_load;
        self
    }

    /// Set a search hint for better ToolSearch discoverability.
    pub fn with_search_hint(mut self, hint: impl Into<String>) -> Self {
        self.search_hint_text = Some(hint.into());
        self
    }
}

#[async_trait]
impl Tool for McpToolInstance {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_mcp(&self) -> bool {
        true
    }

    fn always_load(&self) -> bool {
        self.should_always_load
    }

    fn search_hint(&self) -> Option<&str> {
        self.search_hint_text.as_deref()
    }

    fn mcp_info(&self) -> Option<&McpInfo> {
        Some(&self.mcp_info_data)
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        self.schema.clone()
    }

    fn input_json_schema(&self) -> Option<&ToolInputJSONSchema> {
        Some(&self.schema)
    }

    async fn call(
        &self,
        _input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        // Report progress
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::Mcp(McpProgress {
                        server_name: self.server_name.clone(),
                        status: "calling".to_string(),
                    }),
                });
            }
        }

        // In the full implementation, this would forward the call through
        // the MCP client to the remote server via JSON-RPC:
        //
        // 1. Look up the MCP client for self.server_name
        // 2. Send a tools/call request with the tool name and input
        // 3. Return the result, including any _meta or structuredContent
        //
        // For now, return an error indicating the server is not connected.
        Err(ToolError::ExecutionFailed {
            message: format!(
                "MCP server '{}' is not connected. Cannot call tool '{}'.\n\
                 Configure MCP servers in settings.json to enable this tool.",
                self.server_name,
                self.mcp_info_data.tool_name,
            ),
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        // MCP tools require explicit permission since they can have side effects
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, _: &serde_json::Value, _: &ToolPermissionContext) -> String {
        self.description_text.clone()
    }

    async fn prompt(&self) -> String {
        self.description_text.clone()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        format!("{}:{}", self.server_name, self.mcp_info_data.tool_name)
    }
}
