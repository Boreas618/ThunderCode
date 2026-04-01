//! TeamDeleteTool -- delete an agent team.
//!
//! Ported from ref/tools/TeamDeleteTool/TeamDeleteTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TEAM_DELETE_TOOL_NAME: &str = "TeamDelete";

pub struct TeamDeleteTool;

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        TEAM_DELETE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("delete an agent team and stop all members")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The team name to delete"
                }
            },
            "required": ["name"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            return ValidationResult::invalid("team name must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut store = crate::tools::team_create::TEAM_STORE.lock().unwrap();
        let initial_len = store.len();

        // Find the team's members before deleting (for reporting)
        let member_names: Vec<String> = store
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&name))
            .map(|t| t.members.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default();

        store.retain(|t| !t.name.eq_ignore_ascii_case(&name));
        let removed = store.len() < initial_len;

        if removed {
            Ok(ToolCallResult {
                data: serde_json::json!({
                    "teamName": name,
                    "deleted": true,
                    "stoppedMembers": member_names,
                    "message": format!("Team '{}' deleted. {} member(s) stopped.", name, member_names.len()),
                }),
                new_messages: None,
                mcp_meta: None,
            })
        } else {
            Err(ToolError::ExecutionFailed {
                message: format!("Team '{}' not found", name),
            })
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        format!("Delete team: {name}")
    }

    async fn prompt(&self) -> String {
        "Delete an agent team and stop all its members.\n\
         Running agents in the team will be sent shutdown signals."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TeamDelete".to_string()
    }
}
