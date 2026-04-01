//! ReadMcpResourceTool -- read a specific MCP resource.
//!
//! Ported from ref/tools/ReadMcpResourceTool/ReadMcpResourceTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const READ_MCP_RESOURCE_TOOL_NAME: &str = "ReadMcpResource";

pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        READ_MCP_RESOURCE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("read content from an MCP server resource")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "The MCP server that owns the resource"
                },
                "uri": {
                    "type": "string",
                    "description": "The resource URI to read"
                }
            },
            "required": ["server_name", "uri"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let server = input.get("server_name").and_then(|v| v.as_str()).unwrap_or("");
        if server.is_empty() {
            return ValidationResult::invalid("server_name must not be empty", 9);
        }
        let uri = input.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        if uri.is_empty() {
            return ValidationResult::invalid("uri must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let server_name = input
            .get("server_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let uri = input
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // In the full implementation, this would forward the read request
        // to the appropriate MCP client which communicates with the server.
        Err(ToolError::ExecutionFailed {
            message: format!(
                "MCP server '{}' is not connected. Cannot read resource '{}'.\n\
                 Configure MCP servers in settings to enable this feature.",
                server_name, uri
            ),
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let uri = input.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        format!("Read MCP resource: {uri}")
    }

    async fn prompt(&self) -> String {
        "Read the content of a specific resource from an MCP server.\n\
         Use ListMcpResources first to discover available resources and their URIs."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "ReadMcpResource".to_string()
    }
}
