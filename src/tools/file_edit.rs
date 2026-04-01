//! FileEditTool -- find-and-replace edits on files.
//!
//! Ported from ref/tools/FileEditTool/FileEditTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Constants
// ============================================================================

pub const FILE_EDIT_TOOL_NAME: &str = "Edit";

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditInput {
    /// Absolute path to the file to edit.
    pub file_path: String,
    /// The exact string to find and replace.
    pub old_string: String,
    /// The replacement string.
    pub new_string: String,
    /// If true, replace all occurrences. Default false.
    #[serde(default)]
    pub replace_all: bool,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        FILE_EDIT_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn search_hint(&self) -> Option<&str> {
        Some("modify file contents in place")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences of old_string (default false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let input: FileEditInput =
            serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
                message: format!("Invalid FileEditTool input: {e}"),
            })?;

        let file_path = expand_path(&input.file_path);

        // Read the file.
        let original = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    if input.old_string.is_empty() {
                        // Creating a new file (old_string is empty).
                        return ToolError::ExecutionFailed {
                            message: String::new(), // Will handle below.
                        };
                    }
                    ToolError::ExecutionFailed {
                        message: format!("File does not exist: {}", input.file_path),
                    }
                } else {
                    ToolError::ExecutionFailed {
                        message: format!("Failed to read file: {e}"),
                    }
                }
            });

        let original = match original {
            Ok(content) => content,
            Err(ToolError::ExecutionFailed { message }) if message.is_empty() => {
                // New file creation: old_string is empty, file doesn't exist.
                let dir = Path::new(&file_path).parent();
                if let Some(d) = dir {
                    tokio::fs::create_dir_all(d).await.map_err(|e| {
                        ToolError::ExecutionFailed {
                            message: format!("Failed to create directory: {e}"),
                        }
                    })?;
                }
                tokio::fs::write(&file_path, &input.new_string)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed {
                        message: format!("Failed to write file: {e}"),
                    })?;

                return Ok(ToolCallResult {
                    data: serde_json::json!({
                        "filePath": input.file_path,
                        "created": true,
                        "oldString": "",
                        "newString": input.new_string
                    }),
                    new_messages: None,
                    mcp_meta: None,
                });
            }
            Err(e) => return Err(e),
        };

        // Perform the replacement.
        let updated = if input.replace_all {
            original.replace(&input.old_string, &input.new_string)
        } else {
            original.replacen(&input.old_string, &input.new_string, 1)
        };

        if updated == original {
            return Err(ToolError::ExecutionFailed {
                message: format!(
                    "String to replace not found in file.\nString: {}",
                    input.old_string
                ),
            });
        }

        // Write the updated content.
        tokio::fs::write(&file_path, &updated)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to write file: {e}"),
            })?;

        Ok(ToolCallResult {
            data: serde_json::json!({
                "filePath": input.file_path,
                "oldString": input.old_string,
                "newString": input.new_string,
                "replaceAll": input.replace_all
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

        let old_str = input.get("old_string").and_then(|v| v.as_str());
        let new_str = input.get("new_string").and_then(|v| v.as_str());

        if old_str.is_none() || new_str.is_none() {
            return ValidationResult::invalid("old_string and new_string are required", 2);
        }

        if old_str == new_str {
            return ValidationResult::invalid(
                "No changes to make: old_string and new_string are exactly the same.",
                3,
            );
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
        format!("Edit {path}")
    }

    async fn prompt(&self) -> String {
        "Performs exact string replacements in files.\n\n\
         Usage:\n\
         - You must use your `Read` tool at least once in the conversation before editing.\n\
         - The edit will FAIL if `old_string` is not unique in the file.\n\
         - Use `replace_all` for replacing and renaming strings across the file."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Edit".to_string()
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
        Some(format!("Editing {path}"))
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
    async fn test_edit_file() {
        let tool = FileEditTool;
        let ctx = make_context();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello world").unwrap();
        let input = serde_json::json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "hello",
            "new_string": "goodbye"
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        assert_eq!(result.data["oldString"].as_str().unwrap(), "hello");
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "goodbye world");
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let tool = FileEditTool;
        let ctx = make_context();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "aaa bbb aaa").unwrap();
        let input = serde_json::json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "aaa",
            "new_string": "ccc",
            "replace_all": true
        });
        tool.call(input, &ctx, None).await.unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "ccc bbb ccc");
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let tool = FileEditTool;
        let ctx = make_context();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello").unwrap();
        let input = serde_json::json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "xyz",
            "new_string": "abc"
        });
        let result = tool.call(input, &ctx, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_same_strings() {
        let tool = FileEditTool;
        let ctx = make_context();
        let input = serde_json::json!({
            "file_path": "/tmp/test.txt",
            "old_string": "same",
            "new_string": "same"
        });
        let v = tool.validate_input(&input, &ctx).await;
        assert!(!v.is_valid());
    }
}
