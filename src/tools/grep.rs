//! GrepTool -- search file contents with regex (via ripgrep or regex crate).
//!
//! Ported from ref/tools/GrepTool/GrepTool.ts.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ============================================================================
// Constants
// ============================================================================

pub const GREP_TOOL_NAME: &str = "Grep";
const DEFAULT_HEAD_LIMIT: usize = 250;
const VCS_DIRS: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepInput {
    /// Regex pattern to search for.
    pub pattern: String,
    /// File or directory to search in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Glob filter for files (e.g. "*.js").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glob: Option<String>,
    /// Output mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mode: Option<String>,
    /// Context lines before match.
    #[serde(rename = "-B", skip_serializing_if = "Option::is_none")]
    pub context_before: Option<usize>,
    /// Context lines after match.
    #[serde(rename = "-A", skip_serializing_if = "Option::is_none")]
    pub context_after: Option<usize>,
    /// Context lines before and after.
    #[serde(rename = "-C", skip_serializing_if = "Option::is_none")]
    pub context_c: Option<usize>,
    /// Context lines (alias for -C).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<usize>,
    /// Show line numbers.
    #[serde(rename = "-n", skip_serializing_if = "Option::is_none")]
    pub show_line_numbers: Option<bool>,
    /// Case insensitive.
    #[serde(rename = "-i", skip_serializing_if = "Option::is_none")]
    pub case_insensitive: Option<bool>,
    /// File type filter (e.g. "js", "py").
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    /// Limit output entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_limit: Option<usize>,
    /// Skip first N entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// Enable multiline mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiline: Option<bool>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        GREP_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        20_000
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("search file contents with regex (ripgrep)")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (rg PATH). Defaults to current working directory."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.js\", \"*.{ts,tsx}\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode. Defaults to \"files_with_matches\"."
                },
                "-B": { "type": "number", "description": "Lines before each match" },
                "-A": { "type": "number", "description": "Lines after each match" },
                "-C": { "type": "number", "description": "Alias for context." },
                "context": { "type": "number", "description": "Lines before and after each match" },
                "-n": { "type": "boolean", "description": "Show line numbers. Defaults to true." },
                "-i": { "type": "boolean", "description": "Case insensitive search" },
                "type": { "type": "string", "description": "File type filter (e.g. js, py, rust)" },
                "head_limit": { "type": "number", "description": "Limit output entries. Defaults to 250." },
                "offset": { "type": "number", "description": "Skip first N entries." },
                "multiline": { "type": "boolean", "description": "Enable multiline mode." }
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
        let input: GrepInput =
            serde_json::from_value(input).map_err(|e| ToolError::ValidationFailed {
                message: format!("Invalid GrepTool input: {e}"),
            })?;

        let output_mode = input.output_mode.as_deref().unwrap_or("files_with_matches");
        let case_insensitive = input.case_insensitive.unwrap_or(false);
        let head_limit = input.head_limit.unwrap_or(DEFAULT_HEAD_LIMIT);
        let offset = input.offset.unwrap_or(0);

        // Try to use ripgrep via command line first.
        let search_path = input
            .path
            .as_deref()
            .map(|p| expand_path(p))
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            });

        // Build ripgrep arguments.
        let mut args = vec!["--hidden".to_string()];

        // Exclude VCS directories.
        for dir in VCS_DIRS {
            args.push("--glob".into());
            args.push(format!("!{dir}"));
        }

        args.push("--max-columns".into());
        args.push("500".into());

        if input.multiline.unwrap_or(false) {
            args.push("-U".into());
            args.push("--multiline-dotall".into());
        }

        if case_insensitive {
            args.push("-i".into());
        }

        match output_mode {
            "files_with_matches" => args.push("-l".into()),
            "count" => args.push("-c".into()),
            _ => {}
        }

        let show_line_numbers = input.show_line_numbers.unwrap_or(true);
        if show_line_numbers && output_mode == "content" {
            args.push("-n".into());
        }

        // Context flags.
        if output_mode == "content" {
            let ctx = input.context.or(input.context_c);
            if let Some(c) = ctx {
                args.push("-C".into());
                args.push(c.to_string());
            } else {
                if let Some(b) = input.context_before {
                    args.push("-B".into());
                    args.push(b.to_string());
                }
                if let Some(a) = input.context_after {
                    args.push("-A".into());
                    args.push(a.to_string());
                }
            }
        }

        // Pattern.
        if input.pattern.starts_with('-') {
            args.push("-e".into());
        }
        args.push(input.pattern.clone());

        if let Some(ref ft) = input.file_type {
            args.push("--type".into());
            args.push(ft.clone());
        }

        if let Some(ref g) = input.glob {
            args.push("--glob".into());
            args.push(g.clone());
        }

        args.push(search_path.clone());

        // Try ripgrep.
        let rg_result = tokio::process::Command::new("rg")
            .args(&args)
            .output()
            .await;

        match rg_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout
                    .lines()
                    .filter(|l| !l.is_empty())
                    .collect();

                // Apply pagination.
                let total = lines.len();
                let start = offset.min(total);
                let effective_limit = if head_limit == 0 { total } else { head_limit };
                let end = (start + effective_limit).min(total);
                let paginated = &lines[start..end];
                let was_truncated = total > end;

                let cwd = std::env::current_dir().unwrap_or_default();

                match output_mode {
                    "content" => Ok(ToolCallResult {
                        data: serde_json::json!({
                            "mode": "content",
                            "numFiles": 0,
                            "filenames": [],
                            "content": paginated.join("\n"),
                            "numLines": paginated.len(),
                            "appliedLimit": if was_truncated { Some(effective_limit) } else { None::<usize> },
                            "appliedOffset": if offset > 0 { Some(offset) } else { None::<usize> }
                        }),
                        new_messages: None,
                        mcp_meta: None,
                    }),
                    "count" => {
                        let mut total_matches = 0u64;
                        let mut file_count = 0u64;
                        for line in paginated {
                            if let Some(idx) = line.rfind(':') {
                                if let Ok(count) = line[idx + 1..].parse::<u64>() {
                                    total_matches += count;
                                    file_count += 1;
                                }
                            }
                        }
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "mode": "count",
                                "numFiles": file_count,
                                "filenames": [],
                                "content": paginated.join("\n"),
                                "numMatches": total_matches,
                                "appliedLimit": if was_truncated { Some(effective_limit) } else { None::<usize> },
                                "appliedOffset": if offset > 0 { Some(offset) } else { None::<usize> }
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    }
                    _ => {
                        // files_with_matches: relativize paths.
                        let filenames: Vec<String> = paginated
                            .iter()
                            .map(|p| {
                                let pb = PathBuf::from(p.trim());
                                to_relative_path(&pb, &cwd)
                            })
                            .collect();
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "mode": "files_with_matches",
                                "filenames": filenames,
                                "numFiles": filenames.len(),
                                "appliedLimit": if was_truncated { Some(effective_limit) } else { None::<usize> },
                                "appliedOffset": if offset > 0 { Some(offset) } else { None::<usize> }
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    }
                }
            }
            Err(_) => {
                // Fallback: use regex crate for basic search (ripgrep not available).
                fallback_grep(&input, &search_path, output_mode, head_limit, offset).await
            }
        }
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
        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        if pattern.is_empty() {
            return ValidationResult::invalid("pattern is required", 1);
        }
        // Validate the regex.
        if Regex::new(pattern).is_err() {
            return ValidationResult::invalid(format!("Invalid regex pattern: {pattern}"), 2);
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
            .unwrap_or("(unknown)");
        format!("Search for {pattern}")
    }

    async fn prompt(&self) -> String {
        "A powerful search tool built on ripgrep.\n\n\
         Supports full regex syntax, file glob filtering, and multiple output modes.\n\
         Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \
         \"count\" shows match counts."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Search".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let pattern = input
            .and_then(|i| i.get("pattern"))
            .and_then(|v| v.as_str())
            .unwrap_or("...");
        Some(format!("Searching for {pattern}"))
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchReadInfo {
        SearchReadInfo {
            is_search: true,
            is_read: false,
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

fn to_relative_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Fallback grep using the regex crate (when ripgrep is not available).
async fn fallback_grep(
    input: &GrepInput,
    search_path: &str,
    output_mode: &str,
    head_limit: usize,
    offset: usize,
) -> Result<ToolCallResult, ToolError> {
    let case_insensitive = input.case_insensitive.unwrap_or(false);
    let pattern_str = if case_insensitive {
        format!("(?i){}", input.pattern)
    } else {
        input.pattern.clone()
    };
    let re = Regex::new(&pattern_str).map_err(|e| ToolError::ExecutionFailed {
        message: format!("Invalid regex: {e}"),
    })?;

    let path = Path::new(search_path);
    let mut matching_files: Vec<PathBuf> = Vec::new();

    if path.is_file() {
        if file_matches(&re, path) {
            matching_files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        walk_dir(path, &re, &mut matching_files, 100);
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let filenames: Vec<String> = matching_files
        .iter()
        .skip(offset)
        .take(if head_limit == 0 { usize::MAX } else { head_limit })
        .map(|p| to_relative_path(p, &cwd))
        .collect();

    Ok(ToolCallResult {
        data: serde_json::json!({
            "mode": output_mode,
            "filenames": filenames,
            "numFiles": filenames.len()
        }),
        new_messages: None,
        mcp_meta: None,
    })
}

fn file_matches(re: &Regex, path: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(path) {
        re.is_match(&content)
    } else {
        false
    }
}

fn walk_dir(dir: &Path, re: &Regex, results: &mut Vec<PathBuf>, limit: usize) {
    if results.len() >= limit {
        return;
    }
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if VCS_DIRS.contains(&dir_name) || dir_name == "node_modules" {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if results.len() >= limit {
                return;
            }
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, re, results, limit);
            } else if path.is_file() && file_matches(re, &path) {
                results.push(path);
            }
        }
    }
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
    async fn test_grep_finds_pattern() {
        let tool = GrepTool;
        let ctx = make_context();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello world\nfoo bar\nhello again").unwrap();
        let input = serde_json::json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.call(input, &ctx, None).await.unwrap();
        let num_files = result.data["numFiles"].as_u64().unwrap();
        assert!(num_files >= 1);
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let tool = GrepTool;
        let ctx = make_context();
        let input = serde_json::json!({ "pattern": "[invalid" });
        let v = tool.validate_input(&input, &ctx).await;
        assert!(!v.is_valid());
    }
}
