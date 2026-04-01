//! GlobTool -- find files by glob pattern.
//!
//! Ported from ref/tools/GlobTool/GlobTool.ts.

use async_trait::async_trait;
use glob::glob as glob_iter;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ============================================================================
// Constants
// ============================================================================

pub const GLOB_TOOL_NAME: &str = "Glob";

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobInput {
    /// The glob pattern to match files against.
    pub pattern: String,
    /// Directory to search in. Defaults to cwd.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        GLOB_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("find files by name pattern or wildcard")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. If not specified, the current working directory will be used."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let input: GlobInput =
            serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
                message: format!("Invalid GlobTool input: {e}"),
            })?;

        let start = std::time::Instant::now();
        let base_dir = input
            .path
            .as_deref()
            .map(expand_path)
            .unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string());

        // Build the full glob pattern.
        let full_pattern = if input.pattern.starts_with('/') {
            input.pattern.clone()
        } else {
            format!("{}/{}", base_dir, input.pattern)
        };

        let limit = 100;
        let mut files: Vec<PathBuf> = Vec::new();

        match glob_iter(&full_pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if entry.is_file() {
                        files.push(entry);
                        if files.len() >= limit {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ToolError::ExecutionFailed {
                    message: format!("Invalid glob pattern: {e}"),
                });
            }
        }

        let truncated = files.len() >= limit;

        // Sort by modification time (most recent first).
        files.sort_by(|a, b| {
            let mt_a = a.metadata().and_then(|m| m.modified()).ok();
            let mt_b = b.metadata().and_then(|m| m.modified()).ok();
            mt_b.cmp(&mt_a)
        });

        let cwd = std::env::current_dir().unwrap_or_default();
        let filenames: Vec<String> = files
            .iter()
            .map(|p| to_relative_path(p, &cwd))
            .collect();

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolCallResult {
            data: serde_json::json!({
                "filenames": filenames,
                "durationMs": duration_ms,
                "numFiles": filenames.len(),
                "truncated": truncated
            }),
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => {}
            _ => return ValidationResult::invalid("pattern is required", 1),
        }

        // Validate path if provided.
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            let abs = expand_path(path);
            if !Path::new(&abs).is_dir() {
                return ValidationResult::invalid(
                    format!("Directory does not exist: {path}"),
                    2,
                );
            }
        }

        ValidationResult::valid()
    }

    fn description(
        &self,
        input: &serde_json::Value,
        _tool_permission_context: &ToolPermissionContext,
    ) -> String {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*");
        format!("Glob {pattern}")
    }

    async fn prompt(&self) -> String {
        "Fast file pattern matching tool that works with any codebase size.\n\
         Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\".\n\
         Returns matching file paths sorted by modification time."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Glob".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let pattern = input
            .and_then(|i| i.get("pattern"))
            .and_then(|v| v.as_str())
            .unwrap_or("*");
        Some(format!("Finding {pattern}"))
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| expand_path(s))
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchReadInfo {
        SearchReadInfo {
            is_search: true,
            is_read: false,
            is_list: None,
        }
    }
}

fn expand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn to_relative_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
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
    async fn test_glob_finds_files() {
        let tool = GlobTool;
        let ctx = make_context();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("other.txt"), "text").unwrap();
        let input = serde_json::json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["numFiles"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tool = GlobTool;
        let ctx = make_context();
        let dir = tempfile::tempdir().unwrap();
        let input = serde_json::json!({
            "pattern": "*.nonexistent",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["numFiles"].as_u64().unwrap(), 0);
    }
}
