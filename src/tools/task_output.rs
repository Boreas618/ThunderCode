//! TaskOutputTool -- get output from a running/completed task.
//!
//! Ported from ref/tools/TaskOutputTool/TaskOutputTool.ts.
//! Reads the output file for a background task and returns its contents.
//! For running tasks, streams new output since last read.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_OUTPUT_TOOL_NAME: &str = "TaskOutput";

pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        TASK_OUTPUT_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn always_load(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("get output from running or completed task")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to get output from"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if task_id.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "task_id must not be empty".to_string(),
            });
        }

        // Try to read the output file for this task
        let output_file = format!("/tmp/thundercode-agent-{}.output", task_id);

        // Report progress
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = _context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::TaskOutput(TaskOutputProgress {
                        task_id: task_id.clone(),
                        output: "Reading task output...".to_string(),
                    }),
                });
            }
        }

        let output = match tokio::fs::read_to_string(&output_file).await {
            Ok(content) => content,
            Err(_) => {
                // Also check AppState tasks for background task info
                return Ok(ToolCallResult {
                    data: serde_json::json!({
                        "taskId": task_id,
                        "status": "not_found",
                        "output": "",
                        "message": format!("No output found for task {}. The task may not exist or hasn't produced output yet.", task_id),
                    }),
                    new_messages: None,
                    mcp_meta: None,
                });
            }
        };

        // Truncate if too large
        let max_chars = self.max_result_size_chars();
        let truncated = if output.len() > max_chars {
            let suffix = format!("\n\n... (output truncated, {} total chars)", output.len());
            format!("{}{}", &output[..max_chars - suffix.len()], suffix)
        } else {
            output
        };

        Ok(ToolCallResult {
            data: serde_json::json!({
                "taskId": task_id,
                "output": truncated,
                "outputFile": output_file,
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
        format!("Get output for task #{task_id}")
    }

    async fn prompt(&self) -> String {
        "Get the output from a running or completed background task.\n\
         Returns the content of the task's output file, which includes\n\
         the agent's conversation transcript and tool results."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskOutput".to_string()
    }
}
