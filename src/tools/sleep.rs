//! SleepTool -- wait for a specified duration.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const SLEEP_TOOL_NAME: &str = "Sleep";

pub struct SleepTool;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str { SLEEP_TOOL_NAME }
    fn max_result_size_chars(&self) -> usize { 1_000 }
    fn is_read_only(&self, _: &serde_json::Value) -> bool { true }
    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool { true }
    fn should_defer(&self) -> bool { true }
    fn search_hint(&self) -> Option<&str> { Some("wait for a specified duration") }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "duration_ms": {
                    "type": "number",
                    "description": "Duration to sleep in milliseconds",
                    "minimum": 0,
                    "maximum": 3600000
                }
            },
            "required": ["duration_ms"]
        })
    }

    async fn call(&self, input: serde_json::Value, _: &ToolUseContext, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolCallResult, ToolError> {
        let duration_ms = input.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        let capped = duration_ms.min(3_600_000);
        tokio::time::sleep(std::time::Duration::from_millis(capped)).await;
        Ok(ToolCallResult { data: serde_json::json!({ "slept_ms": capped }), new_messages: None, mcp_meta: None })
    }

    async fn check_permissions(&self, input: &serde_json::Value, _: &ToolUseContext) -> PermissionResult { PermissionResult::allow(Some(input.clone())) }
    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let ms = input.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        format!("Sleep {ms}ms")
    }
    async fn prompt(&self) -> String {
        "Wait for a specified duration. The user can interrupt the sleep at any time.\n\
         Prefer this over `Bash(sleep ...)` -- it doesn't hold a shell process.".to_string()
    }
    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String { "Sleep".to_string() }
}
