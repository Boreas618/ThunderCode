//! TaskGetTool -- get task details.
//!
//! Ported from ref/tools/TaskGetTool/TaskGetTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_GET_TOOL_NAME: &str = "TaskGet";

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        TASK_GET_TOOL_NAME
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
        Some("get task details and status")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to retrieve"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let store = crate::tools::task_create::TASK_STORE.lock().unwrap();
        let task = store.iter().find(|t| t.id == task_id);

        match task {
            Some(task) => Ok(ToolCallResult {
                data: serde_json::json!({
                    "task": {
                        "id": task.id,
                        "subject": task.subject,
                        "description": task.description,
                        "status": task.status,
                        "activeForm": task.active_form,
                        "owner": task.owner,
                        "blocks": task.blocks,
                        "blockedBy": task.blocked_by,
                        "metadata": task.metadata,
                        "createdAt": task.created_at,
                        "updatedAt": task.updated_at,
                    }
                }),
                new_messages: None,
                mcp_meta: None,
            }),
            None => Err(ToolError::ExecutionFailed {
                message: format!("Task not found: {task_id}"),
            }),
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
        let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        format!("Get task #{task_id}")
    }

    async fn prompt(&self) -> String {
        "Get the full details of a specific task by ID, including its status, description, \
         owner, and blocking relationships."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskGet".to_string()
    }
}
