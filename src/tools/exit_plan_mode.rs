//! ExitPlanModeTool -- exit plan mode and resume normal execution.
//!
//! Ported from ref/tools/ExitPlanModeTool/ExitPlanModeTool.ts.
//! The user reviews the plan and approves/rejects. On approval the
//! permission mode transitions back to the user's default mode.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const EXIT_PLAN_MODE_TOOL_NAME: &str = "ExitPlanMode";

pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        EXIT_PLAN_MODE_TOOL_NAME
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

    fn always_load(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("exit planning mode and present plan for approval")
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
        if context.agent_id.is_some() {
            return Err(ToolError::ExecutionFailed {
                message: "ExitPlanMode tool cannot be used in agent contexts".to_string(),
            });
        }

        // The actual mode transition (plan -> default/auto) is handled by the
        // query engine when it processes this tool result. The UI shows the
        // plan to the user for approval before switching.
        Ok(ToolCallResult {
            data: serde_json::json!({
                "message": "Plan mode ended. The plan has been presented to the user for review."
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
        "Present the plan for user review and approval".to_string()
    }

    async fn prompt(&self) -> String {
        "Exit plan mode and present your plan to the user for review.\n\
         \n\
         Call this tool when your plan is complete and ready for the user to review.\n\
         The plan is presented as all of the assistant messages you've sent during plan mode.\n\
         \n\
         After calling this tool, the user will be asked to approve or reject the plan.\n\
         If they approve, you'll exit plan mode and can begin implementing.\n\
         If they reject, you'll remain in plan mode to revise.\n\
         \n\
         IMPORTANT: Do NOT use AskUserQuestion to ask 'Should I proceed?' -- use this tool instead.\n\
         The UI has a dedicated approval flow for plans."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }
}
