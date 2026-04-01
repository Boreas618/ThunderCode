//! BriefTool -- toggle brief/verbose output mode.
//!
//! Ported from ref/tools/BriefTool/BriefTool.ts.
//! When brief mode is enabled, the assistant uses this tool as the primary
//! communication channel, showing only concise messages to the user.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const BRIEF_TOOL_NAME: &str = "Brief";

pub struct BriefTool;

#[async_trait]
impl Tool for BriefTool {
    fn name(&self) -> &str {
        BRIEF_TOOL_NAME
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
        Some("toggle brief or verbose output mode")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message for the user. Supports markdown formatting."
                },
                "attachments": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional file paths (absolute or relative to cwd) to attach."
                },
                "status": {
                    "type": "string",
                    "enum": ["normal", "proactive"],
                    "description": "Use 'proactive' when surfacing something the user hasn't asked for. Use 'normal' when replying to user."
                }
            },
            "required": ["message", "status"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let attachments = input
            .get("attachments")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|path| {
                        let p = std::path::Path::new(path);
                        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                        let ext = p
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let is_image =
                            matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg");
                        serde_json::json!({
                            "path": path,
                            "size": size,
                            "isImage": is_image,
                        })
                    })
                    .collect::<Vec<_>>()
            });

        let sent_at = chrono::Utc::now().to_rfc3339();

        let mut data = serde_json::json!({
            "message": message,
            "sentAt": sent_at,
        });
        if let Some(att) = attachments {
            data["attachments"] = serde_json::json!(att);
        }

        Ok(ToolCallResult {
            data,
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
        "Send a brief message to the user".to_string()
    }

    async fn prompt(&self) -> String {
        "Brief is the primary communication channel. Use this tool to send messages to the user.\n\
         \n\
         Your text output is NOT visible to the user. To communicate, you MUST call this tool.\n\
         EVERY turn where you have something to say requires a Brief call -- otherwise the user sees nothing.\n\
         \n\
         Use 'proactive' status when surfacing something the user hasn't asked for.\n\
         Use 'normal' status when replying to something the user just said."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let msg = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        serde_json::Value::String(msg.to_string())
    }
}
