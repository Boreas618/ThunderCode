//! FileReadTool -- read files with line numbers, offset/limit support.
//!
//! Ported from ref/tools/FileReadTool/FileReadTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Constants
// ============================================================================

pub const FILE_READ_TOOL_NAME: &str = "Read";
const MAX_FILE_READ_TOKENS: usize = 10_000;
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadInput {
    /// Absolute path to the file to read.
    pub file_path: String,
    /// Starting line number (0-based). Only needed for large files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// Number of lines to read. Only needed for large files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Page range for PDF files (e.g., "1-5").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        FILE_READ_TOOL_NAME
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
        Some("read file contents, view images, read PDFs, view notebooks")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "The line number to start reading from. Only provide if the file is too large to read at once",
                    "minimum": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once.",
                    "exclusiveMinimum": 0
                },
                "pages": {
                    "type": "string",
                    "description": "Page range for PDF files (e.g., \"1-5\", \"3\", \"10-20\"). Only applicable to PDF files. Maximum 20 pages per request."
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let input: FileReadInput =
            serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
                message: format!("Invalid FileReadTool input: {e}"),
            })?;

        let file_path = expand_path(&input.file_path);

        // Check if file is an image.
        if is_image_file(&file_path) {
            return read_image(&file_path).await;
        }

        // Read text file.
        let content =
            tokio::fs::read_to_string(&file_path)
                .await
                .map_err(|e| ToolError::ExecutionFailed {
                    message: if e.kind() == std::io::ErrorKind::NotFound {
                        format!("File does not exist: {}", input.file_path)
                    } else {
                        format!("Failed to read file: {e}")
                    },
                })?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply offset/limit.
        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(2000);
        let end = (offset + limit).min(total_lines);
        let slice = &lines[offset.min(total_lines)..end];

        // Add line numbers (1-based, matching cat -n).
        let numbered: Vec<String> = slice
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}\t{}", offset + i + 1, line))
            .collect();

        let result_content = numbered.join("\n");

        // Token count check (rough estimate: 1 token ~= 4 chars).
        let estimated_tokens = result_content.len() / 4;
        if estimated_tokens > MAX_FILE_READ_TOKENS {
            return Err(ToolError::ExecutionFailed {
                message: format!(
                    "File content ({estimated_tokens} tokens) exceeds maximum allowed tokens ({MAX_FILE_READ_TOKENS}). \
                     Use offset and limit parameters to read specific portions of the file, or search for specific content instead of reading the whole file."
                ),
            });
        }

        Ok(ToolCallResult {
            data: serde_json::json!({
                "type": "text",
                "file": {
                    "filePath": input.file_path,
                    "content": result_content,
                    "numLines": slice.len(),
                    "startLine": offset + 1,
                    "totalLines": total_lines
                }
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
        match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => ValidationResult::valid(),
            _ => ValidationResult::invalid("file_path is required and must be a non-empty string", 1),
        }
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
        format!("Read {path}")
    }

    async fn prompt(&self) -> String {
        "Reads a file from the local filesystem. You can access any file directly by using this tool.\n\
         Results are returned using cat -n format, with line numbers starting at 1.\n\
         By default, it reads up to 2000 lines starting from the beginning of the file.\n\
         When you already know which part of the file you need, only read that part."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Read".to_string()
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
        Some(format!("Reading {path}"))
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchReadInfo {
        SearchReadInfo {
            is_search: false,
            is_read: true,
            is_list: None,
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn expand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn is_image_file(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    IMAGE_EXTENSIONS.contains(&ext.as_str())
}

async fn read_image(path: &str) -> Result<ToolCallResult, ToolError> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to read image: {e}"),
        })?;

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let media_type = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };

    let encoded = base64_encode(&data);

    Ok(ToolCallResult {
        data: serde_json::json!({
            "type": "image",
            "file": {
                "base64": encoded,
                "type": media_type,
                "originalSize": encoded.len()
            }
        }),
        new_messages: None,
        mcp_meta: None,
    })
}

/// Simple base64 encoder (avoids external crate dependency).
fn base64_encode(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity((bytes.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as usize;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as usize } else { 0 };
        encoded.push(CHARS[(b0 >> 2) & 0x3f] as char);
        encoded.push(CHARS[((b0 << 4) | (b1 >> 4)) & 0x3f] as char);
        if i + 1 < bytes.len() {
            encoded.push(CHARS[((b1 << 2) | (b2 >> 6)) & 0x3f] as char);
        } else {
            encoded.push('=');
        }
        if i + 2 < bytes.len() {
            encoded.push(CHARS[b2 & 0x3f] as char);
        } else {
            encoded.push('=');
        }
        i += 3;
    }
    encoded
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
    async fn test_read_self() {
        let tool = FileReadTool;
        let ctx = make_context();
        let _schema_check = serde_json::json!({ "file_path": file!() });
        // This file might not exist at the workspace path during test;
        // instead test with a temp file.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut std::io::BufWriter::new(tmp.as_file()), b"line1\nline2\nline3\n").unwrap();
        let input = serde_json::json!({ "file_path": tmp.path().to_str().unwrap() });
        let result = tool.call(input, &ctx, None).await.unwrap();
        let content = result.data["file"]["content"].as_str().unwrap();
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
    }

    #[tokio::test]
    async fn test_read_with_offset_limit() {
        let tool = FileReadTool;
        let ctx = make_context();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut std::io::BufWriter::new(tmp.as_file()),
            b"a\nb\nc\nd\ne\n",
        )
        .unwrap();
        let input = serde_json::json!({
            "file_path": tmp.path().to_str().unwrap(),
            "offset": 1,
            "limit": 2
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        let num_lines = result.data["file"]["numLines"].as_u64().unwrap();
        assert_eq!(num_lines, 2);
    }

    #[tokio::test]
    async fn test_read_nonexistent() {
        let tool = FileReadTool;
        let ctx = make_context();
        let input = serde_json::json!({ "file_path": "/nonexistent/file.txt" });
        let result = tool.call(input, &ctx, None).await;
        assert!(result.is_err());
    }
}
