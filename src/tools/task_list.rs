//! TaskListTool -- list all tasks.
//!
//! Ported from ref/tools/TaskListTool/TaskListTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_LIST_TOOL_NAME: &str = "TaskList";

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        TASK_LIST_TOOL_NAME
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
        Some("list all tasks and their statuses")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "all"],
                    "description": "Filter by status (default: all)"
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
        let status_filter = input
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let store = crate::tools::task_create::TASK_STORE.lock().unwrap();

        let tasks: Vec<serde_json::Value> = store
            .iter()
            .filter(|t| {
                status_filter == "all" || t.status == status_filter
            })
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "subject": t.subject,
                    "status": t.status,
                    "activeForm": t.active_form,
                    "blockedBy": t.blocked_by,
                    "blocks": t.blocks,
                    "createdAt": t.created_at,
                })
            })
            .collect();

        let total = tasks.len();

        Ok(ToolCallResult {
            data: serde_json::json!({
                "tasks": tasks,
                "total": total,
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
        "List tasks".to_string()
    }

    async fn prompt(&self) -> String {
        "List all tasks in the task list, optionally filtered by status.\n\
         Returns a summary of each task including ID, subject, status, and blocking relationships."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskList".to_string()
    }
}
