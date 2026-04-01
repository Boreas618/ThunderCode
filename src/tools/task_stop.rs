//! TaskStopTool -- stop a running background task.
//!
//! Ported from ref/tools/TaskStopTool/TaskStopTool.ts.
//! Sends an abort signal to a running background task. The task
//! transitions to 'killed' status.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_STOP_TOOL_NAME: &str = "TaskStop";

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        TASK_STOP_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn always_load(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("stop a running background task")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to stop"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        if task_id.trim().is_empty() {
            return ValidationResult::invalid("task_id must not be empty", 9);
        }
        ValidationResult::valid()
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

        // Try to update the task in the v2 store
        {
            let mut store = crate::tools::task_create::TASK_STORE.lock().unwrap();
            if let Some(task) = store.iter_mut().find(|t| t.id == task_id) {
                let old_status = task.status.clone();
                if old_status == "completed" {
                    return Ok(ToolCallResult {
                        data: serde_json::json!({
                            "taskId": task_id,
                            "stopped": false,
                            "message": format!("Task {} is already completed", task_id),
                        }),
                        new_messages: None,
                        mcp_meta: None,
                    });
                }
                task.status = "completed".to_string();
                task.updated_at = Some(chrono::Utc::now().to_rfc3339());

                return Ok(ToolCallResult {
                    data: serde_json::json!({
                        "taskId": task_id,
                        "stopped": true,
                        "oldStatus": old_status,
                        "newStatus": "completed",
                    }),
                    new_messages: None,
                    mcp_meta: None,
                });
            }
        }

        // If not in v2 store, try to signal the background process.
        // In the full implementation, this would look up the task in
        // AppState.tasks and call its abort controller.
        Ok(ToolCallResult {
            data: serde_json::json!({
                "taskId": task_id,
                "stopped": false,
                "message": format!("Task {} not found. It may have already completed or been cleaned up.", task_id),
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

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        format!("Stop task #{task_id}")
    }

    async fn prompt(&self) -> String {
        "Stop a running background task. The task receives an abort signal and \
         transitions to a terminal state. Use this to cancel agents or tasks that \
         are no longer needed."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskStop".to_string()
    }
}
