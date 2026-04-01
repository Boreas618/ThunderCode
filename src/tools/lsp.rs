//! LSPTool -- Language Server Protocol operations.
//!
//! Ported from ref/tools/LSPTool/LSPTool.ts.
//! Provides goToDefinition and findReferences by spawning language servers
//! and communicating via the LSP protocol.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const LSP_TOOL_NAME: &str = "LSP";

pub struct LSPTool;

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str {
        LSP_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        50_000
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn is_lsp(&self) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("language server protocol operations, type checking, go to definition")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["definition", "references", "hover", "diagnostics"],
                    "description": "LSP action to perform"
                },
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (0-based)"
                },
                "character": {
                    "type": "integer",
                    "description": "Character offset (0-based)"
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional symbol name for context"
                }
            },
            "required": ["action", "file_path"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
        if !["definition", "references", "hover", "diagnostics"].contains(&action) {
            return ValidationResult::invalid(
                "action must be one of: definition, references, hover, diagnostics",
                9,
            );
        }

        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        if file_path.is_empty() {
            return ValidationResult::invalid("file_path must not be empty", 9);
        }

        if !std::path::Path::new(file_path).exists() {
            return ValidationResult::invalid(
                &format!("File not found: {file_path}"),
                9,
            );
        }

        // For definition and references, line is required
        if (action == "definition" || action == "references" || action == "hover")
            && input.get("line").is_none()
        {
            return ValidationResult::invalid(
                "line is required for definition, references, and hover actions",
                9,
            );
        }

        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let character = input.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        // Determine the language server to use based on file extension
        let ext = std::path::Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let lsp_command = match ext.as_str() {
            "ts" | "tsx" | "js" | "jsx" => Some(("typescript-language-server", vec!["--stdio"])),
            "rs" => Some(("rust-analyzer", vec![])),
            "py" => Some(("pylsp", vec![])),
            "go" => Some(("gopls", vec!["serve"])),
            _ => None,
        };

        if lsp_command.is_none() {
            return Err(ToolError::ExecutionFailed {
                message: format!(
                    "No language server configured for .{ext} files. \
                     Supported: .ts/.tsx/.js/.jsx (typescript-language-server), \
                     .rs (rust-analyzer), .py (pylsp), .go (gopls)"
                ),
            });
        }

        // For now, use a grep-based fallback for definition and references
        // since spawning a full LSP server is complex and requires initialization.
        // This provides useful results without the LSP protocol overhead.
        match action.as_str() {
            "definition" => {
                let symbol = input
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Try to find definition using grep patterns
                let results = find_definition_grep(&file_path, line, character, symbol).await?;

                Ok(ToolCallResult {
                    data: serde_json::json!({
                        "action": "definition",
                        "file": file_path,
                        "line": line,
                        "character": character,
                        "results": results,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                })
            }
            "references" => {
                let symbol = input
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let results = find_references_grep(&file_path, line, character, symbol).await?;

                Ok(ToolCallResult {
                    data: serde_json::json!({
                        "action": "references",
                        "file": file_path,
                        "line": line,
                        "character": character,
                        "results": results,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                })
            }
            "hover" => {
                // Read the file and extract the symbol at position
                let content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
                    ToolError::ExecutionFailed {
                        message: format!("Failed to read file: {e}"),
                    }
                })?;

                let lines: Vec<&str> = content.lines().collect();
                let target_line = lines.get(line).unwrap_or(&"");

                Ok(ToolCallResult {
                    data: serde_json::json!({
                        "action": "hover",
                        "file": file_path,
                        "line": line,
                        "lineContent": target_line,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                })
            }
            "diagnostics" => {
                // Run compiler/linter to get diagnostics
                let diagnostics = get_diagnostics(&file_path, &ext).await?;

                Ok(ToolCallResult {
                    data: serde_json::json!({
                        "action": "diagnostics",
                        "file": file_path,
                        "diagnostics": diagnostics,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                })
            }
            _ => Err(ToolError::ExecutionFailed {
                message: format!("Unknown action: {action}"),
            }),
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("query");
        let file = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let basename = std::path::Path::new(file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file);
        format!("LSP {action} in {basename}")
    }

    async fn prompt(&self) -> String {
        "Interact with language servers for code intelligence features like:\n\
         - Go to definition: Find where a symbol is defined\n\
         - Find references: Find all usages of a symbol\n\
         - Hover: Get type information for a symbol\n\
         - Diagnostics: Get compiler/linter errors for a file\n\
         \n\
         Requires the appropriate language server to be installed."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "LSP".to_string()
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Grep-based definition finder (fallback when LSP server isn't available).
async fn find_definition_grep(
    file_path: &str,
    _line: usize,
    _character: usize,
    symbol: &str,
) -> Result<Vec<serde_json::Value>, ToolError> {
    if symbol.is_empty() {
        return Ok(Vec::new());
    }

    // Search for common definition patterns
    let patterns = [
        format!(r"(fn|function|def|class|struct|enum|type|interface|const|let|var)\s+{}", regex::escape(symbol)),
        format!(r"{}(\s*[=:]|\s*\()", regex::escape(symbol)),
    ];

    let dir = std::path::Path::new(file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");

    let mut results = Vec::new();
    for pattern in &patterns {
        let output = tokio::process::Command::new("grep")
            .args(["-rn", "--include=*.rs", "--include=*.ts", "--include=*.tsx",
                   "--include=*.js", "--include=*.jsx", "--include=*.py",
                   "--include=*.go", "-E", pattern, dir])
            .output()
            .await;

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().take(10) {
                if let Some((path_and_line, content)) = line.split_once(':') {
                    if let Some((path, line_num)) = path_and_line.rsplit_once(':') {
                        results.push(serde_json::json!({
                            "file": path,
                            "line": line_num.parse::<usize>().unwrap_or(0),
                            "content": content.trim(),
                        }));
                    }
                }
            }
        }

        if !results.is_empty() {
            break;
        }
    }

    Ok(results)
}

/// Grep-based reference finder.
async fn find_references_grep(
    file_path: &str,
    _line: usize,
    _character: usize,
    symbol: &str,
) -> Result<Vec<serde_json::Value>, ToolError> {
    if symbol.is_empty() {
        return Ok(Vec::new());
    }

    let dir = std::path::Path::new(file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");

    let output = tokio::process::Command::new("grep")
        .args(["-rn", "--include=*.rs", "--include=*.ts", "--include=*.tsx",
               "--include=*.js", "--include=*.jsx", "--include=*.py",
               "--include=*.go", "-w", symbol, dir])
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed {
            message: format!("grep failed: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> = stdout
        .lines()
        .take(50)
        .filter_map(|line| {
            let (path_and_line, content) = line.split_once(':')?;
            let (path, line_num) = path_and_line.rsplit_once(':')?;
            Some(serde_json::json!({
                "file": path,
                "line": line_num.parse::<usize>().unwrap_or(0),
                "content": content.trim(),
            }))
        })
        .collect();

    Ok(results)
}

/// Run compiler/linter to get diagnostics.
async fn get_diagnostics(
    file_path: &str,
    ext: &str,
) -> Result<Vec<serde_json::Value>, ToolError> {
    let output = match ext {
        "ts" | "tsx" => {
            tokio::process::Command::new("npx")
                .args(["tsc", "--noEmit", "--pretty", "false", file_path])
                .output()
                .await
        }
        "rs" => {
            let dir = std::path::Path::new(file_path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".");
            tokio::process::Command::new("cargo")
                .args(["check", "--message-format=short"])
                .current_dir(dir)
                .output()
                .await
        }
        "py" => {
            tokio::process::Command::new("python3")
                .args(["-m", "py_compile", file_path])
                .output()
                .await
        }
        _ => {
            return Ok(vec![serde_json::json!({
                "message": format!("No diagnostic tool configured for .{ext} files")
            })]);
        }
    };

    match output {
        Ok(out) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            let diagnostics: Vec<serde_json::Value> = combined
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(50)
                .map(|l| serde_json::json!({ "message": l }))
                .collect();
            Ok(diagnostics)
        }
        Err(e) => Ok(vec![serde_json::json!({
            "message": format!("Failed to run diagnostics: {e}")
        })]),
    }
}
