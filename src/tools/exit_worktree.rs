//! ExitWorktreeTool -- leave and optionally clean up a git worktree.
//!
//! Ported from ref/tools/ExitWorktreeTool/ExitWorktreeTool.ts.
//! Leaves the current worktree and optionally removes it if no
//! changes were made.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const EXIT_WORKTREE_TOOL_NAME: &str = "ExitWorktree";

pub struct ExitWorktreeTool;

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        EXIT_WORKTREE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("leave git worktree and return to main directory")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "cleanup": {
                    "type": "boolean",
                    "description": "Whether to remove the worktree. Defaults to true if no changes were made.",
                    "default": true
                },
                "worktree_path": {
                    "type": "string",
                    "description": "Path to the worktree to exit. If omitted, exits the current worktree."
                }
            }
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let cleanup = input
            .get("cleanup")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let worktree_path = input
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Determine the worktree to exit
        let path = match worktree_path {
            Some(p) => p,
            None => {
                // Try to detect current worktree
                let output = tokio::process::Command::new("git")
                    .args(["rev-parse", "--show-toplevel"])
                    .output()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed {
                        message: format!("Failed to detect worktree: {e}"),
                    })?;
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
        };

        // Check if the worktree has uncommitted changes
        let has_changes = {
            let output = tokio::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&path)
                .output()
                .await
                .map_err(|e| ToolError::ExecutionFailed {
                    message: format!("Failed to check worktree status: {e}"),
                })?;
            let status = String::from_utf8_lossy(&output.stdout);
            !status.trim().is_empty()
        };

        // Get the branch name
        let branch = tokio::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .await
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let should_cleanup = cleanup && !has_changes;

        if should_cleanup {
            // Find the main repo root (parent of worktree dir structure)
            let main_root = tokio::process::Command::new("git")
                .args(["worktree", "list", "--porcelain"])
                .output()
                .await
                .ok()
                .and_then(|o| {
                    let text = String::from_utf8_lossy(&o.stdout);
                    text.lines()
                        .find(|l| l.starts_with("worktree "))
                        .map(|l| l.strip_prefix("worktree ").unwrap_or("").to_string())
                });

            if let Some(root) = main_root {
                // Remove the worktree
                let output = tokio::process::Command::new("git")
                    .args(["worktree", "remove", &path, "--force"])
                    .current_dir(&root)
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => {
                        // Also try to delete the branch
                        let _ = tokio::process::Command::new("git")
                            .args(["branch", "-D", &branch])
                            .current_dir(&root)
                            .output()
                            .await;
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        tracing::warn!("Failed to remove worktree: {stderr}");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to remove worktree: {e}");
                    }
                }
            }
        }

        Ok(ToolCallResult {
            data: serde_json::json!({
                "exited": true,
                "worktreePath": path,
                "branch": branch,
                "hadChanges": has_changes,
                "cleanedUp": should_cleanup,
                "message": if has_changes {
                    format!("Exited worktree at {}. Changes were found on branch '{}' -- worktree preserved.", path, branch)
                } else if should_cleanup {
                    format!("Exited and cleaned up worktree at {} (no changes made).", path)
                } else {
                    format!("Exited worktree at {}.", path)
                }
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
        "Exit worktree and return to main directory".to_string()
    }

    async fn prompt(&self) -> String {
        "Leave the current git worktree and return to the main working directory.\n\
         If no changes were made, the worktree is automatically cleaned up.\n\
         If changes exist, the worktree and branch are preserved for later merging."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "ExitWorktree".to_string()
    }
}
