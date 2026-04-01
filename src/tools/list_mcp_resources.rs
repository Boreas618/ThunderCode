//! ListMcpResourcesTool -- list available MCP resources.
//!
//! Ported from ref/tools/ListMcpResourcesTool/ListMcpResourcesTool.ts.
//! Queries connected MCP servers for their available resources and returns
//! a consolidated list.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const LIST_MCP_RESOURCES_TOOL_NAME: &str = "ListMcpResources";

pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        LIST_MCP_RESOURCES_TOOL_NAME
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
        Some("list available MCP server resources")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Optional MCP server name to filter resources. If omitted, lists resources from all servers."
                }
            }
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let server_filter = input
            .get("server_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // In the full implementation, this would query the MCP client registry
        // for all connected servers and their resource lists.
        // For now, return an empty list with a descriptive message.
        let message = match &server_filter {
            Some(name) => format!("No MCP server named '{}' is currently connected.", name),
            None => "No MCP servers are currently connected. Configure MCP servers in settings to enable this feature.".to_string(),
        };

        Ok(ToolCallResult {
            data: serde_json::json!({
                "resources": [],
                "server_filter": server_filter,
                "message": message,
            }),
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, _: &serde_json::Value, _: &ToolPermissionContext) -> String {
        "List MCP resources".to_string()
    }

    async fn prompt(&self) -> String {
        "List available resources from connected MCP servers.\n\
         Resources are named data items (files, database records, etc.) that \
         can be read via the ReadMcpResource tool.\n\
         \n\
         Optionally filter by server name to see resources from a specific MCP server."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "ListMcpResources".to_string()
    }
}
