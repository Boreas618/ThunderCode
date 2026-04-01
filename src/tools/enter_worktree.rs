//! EnterWorktreeTool -- create a git worktree for isolated work.
//!
//! Ported from ref/tools/EnterWorktreeTool/EnterWorktreeTool.ts.
//! Creates a temporary git worktree so the agent can make changes
//! without affecting the main working directory.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const ENTER_WORKTREE_TOOL_NAME: &str = "EnterWorktree";

pub struct EnterWorktreeTool;

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        ENTER_WORKTREE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("create git worktree for isolated changes")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch name for the worktree. A unique name is generated if omitted."
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
        let branch = input
            .get("branch")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                format!("worktree-{}", nanoid::nanoid!(6, &nanoid::alphabet::SAFE.iter().cloned().collect::<Vec<_>>()))
            });

        // Find the git root
        let git_root = find_git_root().await?;

        // Create the worktree path
        let worktree_path = format!("{}/../.thundercode-worktrees/{}", git_root, branch);

        // Create the worktree directory
        tokio::fs::create_dir_all(
            std::path::Path::new(&worktree_path)
                .parent()
                .unwrap_or(std::path::Path::new("/tmp")),
        )
        .await
        .map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to create worktree directory: {e}"),
        })?;

        // Run git worktree add
        let output = tokio::process::Command::new("git")
            .args(["worktree", "add", "-b", &branch, &worktree_path])
            .current_dir(&git_root)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to run git worktree add: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If branch already exists, try without -b
            if stderr.contains("already exists") {
                let output2 = tokio::process::Command::new("git")
                    .args(["worktree", "add", &worktree_path, &branch])
                    .current_dir(&git_root)
                    .output()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed {
                        message: format!("Failed to run git worktree add: {e}"),
                    })?;

                if !output2.status.success() {
                    let stderr2 = String::from_utf8_lossy(&output2.stderr);
                    return Err(ToolError::ExecutionFailed {
                        message: format!("git worktree add failed: {stderr2}"),
                    });
                }
            } else {
                return Err(ToolError::ExecutionFailed {
                    message: format!("git worktree add failed: {stderr}"),
                });
            }
        }

        Ok(ToolCallResult {
            data: serde_json::json!({
                "branch": branch,
                "worktreePath": worktree_path,
                "gitRoot": git_root,
                "created": true,
                "message": format!("Worktree created at {} on branch {}", worktree_path, branch),
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
        "Create an isolated git worktree".to_string()
    }

    async fn prompt(&self) -> String {
        "Create an isolated git worktree for making changes without affecting the main working directory.\n\
         The worktree gets its own branch and working copy. Changes can be merged back to main later.\n\
         \n\
         This is useful when:\n\
         - You want to make experimental changes without risk\n\
         - An agent needs to work in isolation\n\
         - You want to work on multiple branches simultaneously"
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "EnterWorktree".to_string()
    }
}

async fn find_git_root() -> Result<String, ToolError> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to find git root: {e}"),
        })?;

    if !output.status.success() {
        return Err(ToolError::ExecutionFailed {
            message: "Not in a git repository".to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
