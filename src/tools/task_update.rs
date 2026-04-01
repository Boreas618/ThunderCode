//! TaskUpdateTool -- update a task's status or details.
//!
//! Ported from ref/tools/TaskUpdateTool/TaskUpdateTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_UPDATE_TOOL_NAME: &str = "TaskUpdate";

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        TASK_UPDATE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("update task status or details")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed"],
                    "description": "New status"
                },
                "subject": {
                    "type": "string",
                    "description": "Updated subject"
                },
                "description": {
                    "type": "string",
                    "description": "Updated description"
                },
                "blockedBy": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that block this task"
                },
                "blocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that this task blocks"
                },
                "metadata": {
                    "type": "object",
                    "description": "Metadata to merge into the task"
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

        let mut store = crate::tools::task_create::TASK_STORE.lock().unwrap();
        let task = store.iter_mut().find(|t| t.id == task_id);

        match task {
            Some(task) => {
                let old_status = task.status.clone();

                if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
                    task.status = status.to_string();
                }
                if let Some(subject) = input.get("subject").and_then(|v| v.as_str()) {
                    task.subject = subject.to_string();
                }
                if let Some(description) = input.get("description").and_then(|v| v.as_str()) {
                    task.description = description.to_string();
                }
                if let Some(blocked_by) = input.get("blockedBy").and_then(|v| v.as_array()) {
                    task.blocked_by = blocked_by
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                }
                if let Some(blocks) = input.get("blocks").and_then(|v| v.as_array()) {
                    task.blocks = blocks
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                }
                if let Some(metadata) = input.get("metadata") {
                    if let Some(existing) = &mut task.metadata {
                        if let (Some(existing_obj), Some(new_obj)) =
                            (existing.as_object_mut(), metadata.as_object())
                        {
                            for (k, v) in new_obj {
                                existing_obj.insert(k.clone(), v.clone());
                            }
                        }
                    } else {
                        task.metadata = Some(metadata.clone());
                    }
                }

                task.updated_at = Some(chrono::Utc::now().to_rfc3339());

                Ok(ToolCallResult {
                    data: serde_json::json!({
                        "task": {
                            "id": task.id,
                            "subject": task.subject,
                            "status": task.status,
                            "oldStatus": old_status,
                        },
                        "updated": true,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                })
            }
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
        let status = input.get("status").and_then(|v| v.as_str());
        match status {
            Some(s) => format!("Update task #{task_id} -> {s}"),
            None => format!("Update task #{task_id}"),
        }
    }

    async fn prompt(&self) -> String {
        "Update an existing task's status, subject, description, or blocking relationships.\n\n\
         Use this to:\n\
         - Mark a task as in_progress when you start working on it\n\
         - Mark a task as completed when done\n\
         - Update the description as requirements evolve\n\
         - Set blockedBy to indicate task dependencies"
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskUpdate".to_string()
    }
}
