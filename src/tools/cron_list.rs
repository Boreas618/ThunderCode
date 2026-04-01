//! CronListTool -- list all cron jobs.
//!
//! Ported from ref/tools/ScheduleCronTool/CronListTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const CRON_LIST_TOOL_NAME: &str = "CronList";

pub struct CronListTool;

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str {
        CRON_LIST_TOOL_NAME
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
        Some("list all scheduled cron jobs")
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
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let store = crate::tools::cron_create::CRON_STORE.lock().unwrap();

        let crons: Vec<serde_json::Value> = store
            .iter()
            .map(|job| {
                serde_json::json!({
                    "id": job.id,
                    "schedule": job.schedule,
                    "command": job.command,
                    "description": job.description,
                    "enabled": job.enabled,
                    "runCount": job.run_count,
                    "lastRun": job.last_run,
                    "createdAt": job.created_at,
                })
            })
            .collect();

        let total = crons.len();

        Ok(ToolCallResult {
            data: serde_json::json!({
                "crons": crons,
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
        "List cron jobs".to_string()
    }

    async fn prompt(&self) -> String {
        "List all scheduled cron jobs and their current status.".to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "CronList".to_string()
    }
}
