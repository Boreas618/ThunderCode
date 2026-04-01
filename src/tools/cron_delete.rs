//! CronDeleteTool -- delete a cron job.
//!
//! Ported from ref/tools/ScheduleCronTool/CronDeleteTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const CRON_DELETE_TOOL_NAME: &str = "CronDelete";

pub struct CronDeleteTool;

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> &str {
        CRON_DELETE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("delete a scheduled cron job")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "cron_id": {
                    "type": "string",
                    "description": "The cron job ID to delete"
                }
            },
            "required": ["cron_id"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let id = input.get("cron_id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            return ValidationResult::invalid("cron_id must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let cron_id = input
            .get("cron_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut store = crate::tools::cron_create::CRON_STORE.lock().unwrap();
        let initial_len = store.len();
        store.retain(|job| job.id != cron_id);
        let removed = store.len() < initial_len;

        if removed {
            Ok(ToolCallResult {
                data: serde_json::json!({
                    "cronId": cron_id,
                    "deleted": true,
                    "message": format!("Cron job {} deleted", cron_id),
                }),
                new_messages: None,
                mcp_meta: None,
            })
        } else {
            Err(ToolError::ExecutionFailed {
                message: format!("Cron job {} not found", cron_id),
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
        let id = input.get("cron_id").and_then(|v| v.as_str()).unwrap_or("");
        format!("Delete cron job {id}")
    }

    async fn prompt(&self) -> String {
        "Delete a scheduled cron job by ID. Use CronList to find available job IDs.".to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "CronDelete".to_string()
    }
}
