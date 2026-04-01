//! EnterPlanModeTool -- switch to plan mode.
//!
//! Ported from ref/tools/EnterPlanModeTool/EnterPlanModeTool.ts.
//! Transitions the permission mode to 'plan', restricting tool use to
//! read-only operations while the assistant designs an approach.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const ENTER_PLAN_MODE_TOOL_NAME: &str = "EnterPlanMode";

pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        ENTER_PLAN_MODE_TOOL_NAME
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
        Some("switch to plan mode to design an approach before coding")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(
        &self,
        _input: serde_json::Value,
        context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        // Agents cannot enter plan mode
        if context.agent_id.is_some() {
            return Err(ToolError::ExecutionFailed {
                message: "EnterPlanMode tool cannot be used in agent contexts".to_string(),
            });
        }

        // In the full implementation, this would update AppState to set
        // permission_mode to Plan via the store. For now we signal the
        // transition in the result so the caller (query engine) can apply it.
        Ok(ToolCallResult {
            data: serde_json::json!({
                "message": "Entered plan mode. You should now focus on exploring the codebase and designing an implementation approach."
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
        "Requests permission to enter plan mode for complex tasks requiring exploration and design"
            .to_string()
    }

    async fn prompt(&self) -> String {
        "Use this tool to enter plan mode when you need to explore and design before making changes.\n\
         \n\
         In plan mode, you should:\n\
         - Explore the codebase to understand the architecture\n\
         - Identify the files that need to be modified\n\
         - Design your implementation approach\n\
         - Present your plan to the user for review\n\
         \n\
         IMPORTANT: Plan mode restricts you to read-only tools. You cannot modify files until you exit plan mode.\n\
         \n\
         When your plan is ready, use ExitPlanMode to present it. The user will review and either approve or request changes.\n\
         \n\
         Do NOT enter plan mode for simple, straightforward tasks. Only use it when:\n\
         - The task is complex and requires understanding multiple parts of the codebase\n\
         - You're unsure about the best approach and want to explore options\n\
         - The user explicitly asks you to plan first"
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }
}
