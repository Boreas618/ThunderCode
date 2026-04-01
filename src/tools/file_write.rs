//! FileWriteTool -- create or overwrite files.
//!
//! Ported from ref/tools/FileWriteTool/FileWriteTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Constants
// ============================================================================

pub const FILE_WRITE_TOOL_NAME: &str = "Write";

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteInput {
    /// The absolute path to the file to write.
    pub file_path: String,
    /// The content to write to the file.
    pub content: String,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        FILE_WRITE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn search_hint(&self) -> Option<&str> {
        Some("create or overwrite files")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write (must be absolute, not relative)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let input: FileWriteInput =
            serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
                message: format!("Invalid FileWriteTool input: {e}"),
            })?;

        let file_path = expand_path(&input.file_path);

        // Check if file already exists.
        let is_update = Path::new(&file_path).exists();

        // Ensure parent directory exists.
        if let Some(parent) = Path::new(&file_path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::ExecutionFailed {
                    message: format!("Failed to create directory: {e}"),
                })?;
        }

        // Write the file.
        tokio::fs::write(&file_path, &input.content)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to write file: {e}"),
            })?;

        let write_type = if is_update { "update" } else { "create" };
        let message = if is_update {
            format!("The file {} has been updated successfully.", input.file_path)
        } else {
            format!("File created successfully at: {}", input.file_path)
        };

        Ok(ToolCallResult {
            data: serde_json::json!({
                "type": write_type,
                "filePath": input.file_path,
                "content": input.content,
                "message": message
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
        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        if file_path.is_empty() {
            return ValidationResult::invalid("file_path is required", 1);
        }
        if input.get("content").is_none() {
            return ValidationResult::invalid("content is required", 2);
        }
        ValidationResult::valid()
    }

    fn description(
        &self,
        input: &serde_json::Value,
        _tool_permission_context: &ToolPermissionContext,
    ) -> String {
        let path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");
        format!("Write {path}")
    }

    async fn prompt(&self) -> String {
        "Writes a file to the local filesystem.\n\n\
         Usage:\n\
         - This tool will overwrite the existing file if there is one at the provided path.\n\
         - If this is an existing file, you MUST use the Read tool first.\n\
         - Prefer the Edit tool for modifying existing files."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Write".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let path = input
            .and_then(|i| i.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
            })
            .unwrap_or("file");
        Some(format!("Writing {path}"))
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
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
    async fn test_write_new_file() {
        let tool = FileWriteTool;
        let ctx = make_context();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_file.txt");
        let input = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "content": "hello world"
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["type"].as_str().unwrap(), "create");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_overwrite() {
        let tool = FileWriteTool;
        let ctx = make_context();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "old").unwrap();
        let input = serde_json::json!({
            "file_path": tmp.path().to_str().unwrap(),
            "content": "new"
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["type"].as_str().unwrap(), "update");
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "new");
    }
}
