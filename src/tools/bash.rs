//! BashTool -- execute shell commands.
//!
//! Ported from ref/tools/BashTool/BashTool.tsx.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

pub const BASH_TOOL_NAME: &str = "Bash";
const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes
const MAX_TIMEOUT_MS: u64 = 600_000; // 10 minutes

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashInput {
    /// The bash command to execute.
    pub command: String,
    /// Optional timeout in milliseconds (max 600000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// Optional description of what this command does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Run in background (non-blocking).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_in_background: Option<bool>,
    /// Set true to bypass sandbox mode (requires permission).
    #[serde(rename = "dangerouslyDisableSandbox")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangerously_disable_sandbox: Option<bool>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        BASH_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        1_000_000
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn search_hint(&self) -> Option<&str> {
        Some("execute shell commands, run scripts, git operations")
    }

    fn is_read_only(&self, input: &serde_json::Value) -> bool {
        // Bash commands are generally not read-only; we'd need to parse the
        // command to determine this, which the TS version does via
        // commandSemantics. For now, conservatively return false.
        let _ = input;
        false
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)",
                    "maximum": MAX_TIMEOUT_MS
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does in active voice."
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Set to true to run this command in the background."
                },
                "dangerouslyDisableSandbox": {
                    "type": "boolean",
                    "description": "Set this to true to dangerously override sandbox mode."
                }
            },
            "required": ["command"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let input: BashInput = serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
            message: format!("Invalid BashTool input: {e}"),
        })?;

        let timeout_ms = input
            .timeout
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        let is_background = input.run_in_background.unwrap_or(false);

        // Build the command -- use bash -c for consistent behavior.
        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg("-c").arg(&input.command);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        if is_background {
            // Spawn the process without waiting.
            let child = cmd.spawn().map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to spawn background command: {e}"),
            })?;
            let pid = child.id().unwrap_or(0);

            return Ok(ToolCallResult {
                data: serde_json::json!({
                    "stdout": format!("Background process started with PID {pid}"),
                    "stderr": "",
                    "exit_code": 0,
                    "is_background": true
                }),
                new_messages: None,
                mcp_meta: None,
            });
        }

        // Run with timeout.
        let result =
            tokio::time::timeout(timeout_duration, run_command_to_completion(cmd, &on_progress))
                .await;

        match result {
            Ok(Ok(output)) => Ok(ToolCallResult {
                data: serde_json::json!({
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                    "exit_code": output.exit_code
                }),
                new_messages: None,
                mcp_meta: None,
            }),
            Ok(Err(e)) => Err(ToolError::ExecutionFailed {
                message: e.to_string(),
            }),
            Err(_) => Err(ToolError::Timeout { timeout_ms }),
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> PermissionResult {
        // In the full implementation this would check bash command rules,
        // sandbox config, destructive commands, etc.
        PermissionResult::allow(Some(input.clone()))
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        match input.get("command").and_then(|v| v.as_str()) {
            Some(cmd) if !cmd.is_empty() => ValidationResult::valid(),
            _ => ValidationResult::invalid("command is required and must be a non-empty string", 1),
        }
    }

    fn description(
        &self,
        input: &serde_json::Value,
        _tool_permission_context: &ToolPermissionContext,
    ) -> String {
        let cmd = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("(no command)");
        format!("Run command: {cmd}")
    }

    async fn prompt(&self) -> String {
        "Executes a given bash command and returns its output.\n\n\
         The working directory persists between commands, but shell state does not."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Bash".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let cmd = input
            .and_then(|i| i.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let truncated = if cmd.len() > 60 {
            format!("{}...", &cmd[..57])
        } else {
            cmd.to_string()
        };
        Some(format!("Running `{truncated}`"))
    }

    fn get_path(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    fn is_search_or_read_command(&self, input: &serde_json::Value) -> SearchReadInfo {
        let cmd = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let first_word = cmd.split_whitespace().next().unwrap_or("");
        let search_cmds = ["find", "grep", "rg", "ag", "ack", "locate", "which", "whereis"];
        let read_cmds = ["cat", "head", "tail", "less", "more", "wc", "stat", "file"];
        let list_cmds = ["ls", "tree", "du"];
        SearchReadInfo {
            is_search: search_cmds.contains(&first_word),
            is_read: read_cmds.contains(&first_word),
            is_list: Some(list_cmds.contains(&first_word)),
        }
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

async fn run_command_to_completion(
    mut cmd: tokio::process::Command,
    on_progress: &Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
) -> Result<CommandOutput, anyhow::Error> {
    let child = cmd.spawn()?;

    // Collect output.
    let output = child.wait_with_output().await?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    // Report progress if handler provided (for live output streaming).
    if let Some(on_progress) = on_progress {
        on_progress(ToolProgress {
            tool_use_id: String::new(),
            data: ToolProgressData::Bash(BashProgress {
                stdout: Some(stdout.clone()),
                stderr: Some(stderr.clone()),
            }),
        });
    }

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context() -> ToolUseContext {
        ToolUseContext {
            options: ToolOptions {
                commands: vec![],
                debug: false,
                main_loop_model: "test".into(),
                verbose: false,
                is_non_interactive_session: false,
                max_budget_usd: None,
                custom_system_prompt: None,
                append_system_prompt: None,
                query_source: None,
            },
            messages: vec![],
            agent_id: None,
            agent_type: None,
            file_reading_limits: None,
            glob_limits: None,
            query_tracking: None,
            tool_use_id: None,
            preserve_tool_use_results: None,
        }
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = BashTool;
        let ctx = make_context();
        let input = serde_json::json!({ "command": "echo hello" });
        let result = tool.call(input, &ctx, None).await.unwrap();
        let stdout = result.data["stdout"].as_str().unwrap();
        assert!(stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_exit_code() {
        let tool = BashTool;
        let ctx = make_context();
        let input = serde_json::json!({ "command": "exit 42" });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["exit_code"].as_i64().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_bash_validation_empty() {
        let tool = BashTool;
        let ctx = make_context();
        let input = serde_json::json!({ "command": "" });
        let v = tool.validate_input(&input, &ctx).await;
        assert!(!v.is_valid());
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool;
        let ctx = make_context();
        let input = serde_json::json!({ "command": "sleep 60", "timeout": 100 });
        let result = tool.call(input, &ctx, None).await;
        assert!(matches!(result, Err(ToolError::Timeout { .. })));
    }
}
