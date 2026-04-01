//! NotebookEditTool -- edit Jupyter notebook cells.
//!
//! Ported from ref/tools/NotebookEditTool/NotebookEditTool.ts.
//! Reads and modifies .ipynb files by editing individual cells.
//! Supports inserting, replacing, and deleting cells.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const NOTEBOOK_EDIT_TOOL_NAME: &str = "NotebookEdit";

pub struct NotebookEditTool;

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        NOTEBOOK_EDIT_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("edit Jupyter notebook cells")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "notebook_path": {
                    "type": "string",
                    "description": "Absolute path to the .ipynb file"
                },
                "command": {
                    "type": "string",
                    "enum": ["edit_cell", "insert_cell", "delete_cell"],
                    "description": "The operation to perform"
                },
                "cell_number": {
                    "type": "integer",
                    "description": "The cell number to operate on (0-indexed)"
                },
                "new_source": {
                    "type": "string",
                    "description": "New cell source content (for edit_cell and insert_cell)"
                },
                "cell_type": {
                    "type": "string",
                    "enum": ["code", "markdown"],
                    "description": "Cell type for insert_cell (default: code)",
                    "default": "code"
                }
            },
            "required": ["notebook_path", "command"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let path = input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if path.is_empty() {
            return ValidationResult::invalid("notebook_path must not be empty", 9);
        }
        if !path.ends_with(".ipynb") {
            return ValidationResult::invalid("notebook_path must be a .ipynb file", 9);
        }

        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !["edit_cell", "insert_cell", "delete_cell"].contains(&command) {
            return ValidationResult::invalid(
                "command must be one of: edit_cell, insert_cell, delete_cell",
                9,
            );
        }

        if (command == "edit_cell" || command == "insert_cell")
            && input.get("new_source").is_none()
        {
            return ValidationResult::invalid(
                "new_source is required for edit_cell and insert_cell commands",
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
        let path = input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cell_number = input
            .get("cell_number")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let new_source = input
            .get("new_source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cell_type = input
            .get("cell_type")
            .and_then(|v| v.as_str())
            .unwrap_or("code")
            .to_string();

        // Read the notebook file
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            ToolError::ExecutionFailed {
                message: format!("Failed to read notebook: {e}"),
            }
        })?;

        let mut notebook: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to parse notebook JSON: {e}"),
            })?;

        let cells = notebook
            .get_mut("cells")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| ToolError::ExecutionFailed {
                message: "Notebook has no 'cells' array".to_string(),
            })?;

        let total_cells = cells.len();

        match command.as_str() {
            "edit_cell" => {
                if cell_number >= total_cells {
                    return Err(ToolError::ExecutionFailed {
                        message: format!(
                            "Cell number {cell_number} out of range (notebook has {total_cells} cells)"
                        ),
                    });
                }

                let cell = &mut cells[cell_number];
                // Split source into lines for the notebook format
                let source_lines: Vec<serde_json::Value> = new_source
                    .lines()
                    .enumerate()
                    .map(|(i, line)| {
                        let total = new_source.lines().count();
                        if i < total - 1 {
                            serde_json::Value::String(format!("{line}\n"))
                        } else {
                            serde_json::Value::String(line.to_string())
                        }
                    })
                    .collect();

                cell["source"] = serde_json::json!(source_lines);
                // Clear outputs when editing a code cell
                if cell.get("cell_type").and_then(|v| v.as_str()) == Some("code") {
                    cell["outputs"] = serde_json::json!([]);
                    cell["execution_count"] = serde_json::Value::Null;
                }
            }
            "insert_cell" => {
                let insert_at = cell_number.min(total_cells);
                let source_lines: Vec<serde_json::Value> = new_source
                    .lines()
                    .enumerate()
                    .map(|(i, line)| {
                        let total = new_source.lines().count();
                        if i < total - 1 {
                            serde_json::Value::String(format!("{line}\n"))
                        } else {
                            serde_json::Value::String(line.to_string())
                        }
                    })
                    .collect();

                let new_cell = if cell_type == "markdown" {
                    serde_json::json!({
                        "cell_type": "markdown",
                        "metadata": {},
                        "source": source_lines,
                    })
                } else {
                    serde_json::json!({
                        "cell_type": "code",
                        "execution_count": null,
                        "metadata": {},
                        "outputs": [],
                        "source": source_lines,
                    })
                };

                cells.insert(insert_at, new_cell);
            }
            "delete_cell" => {
                if cell_number >= total_cells {
                    return Err(ToolError::ExecutionFailed {
                        message: format!(
                            "Cell number {cell_number} out of range (notebook has {total_cells} cells)"
                        ),
                    });
                }
                cells.remove(cell_number);
            }
            _ => {
                return Err(ToolError::ExecutionFailed {
                    message: format!("Unknown command: {command}"),
                });
            }
        }

        // Write the notebook back
        let output =
            serde_json::to_string_pretty(&notebook).map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to serialize notebook: {e}"),
            })?;

        tokio::fs::write(&path, &output).await.map_err(|e| {
            ToolError::ExecutionFailed {
                message: format!("Failed to write notebook: {e}"),
            }
        })?;

        let new_total = notebook
            .get("cells")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        Ok(ToolCallResult {
            data: serde_json::json!({
                "notebookPath": path,
                "command": command,
                "cellNumber": cell_number,
                "totalCells": new_total,
                "edited": true,
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

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("edit");
        let path = input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let basename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path);
        format!("{command} in {basename}")
    }

    async fn prompt(&self) -> String {
        "Edit Jupyter notebook (.ipynb) cells. Supports:\n\
         - edit_cell: Replace the content of an existing cell\n\
         - insert_cell: Insert a new cell at a specific position\n\
         - delete_cell: Remove a cell from the notebook\n\
         \n\
         Cell numbers are 0-indexed. Cell types can be 'code' or 'markdown'."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "NotebookEdit".to_string()
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<String> {
        input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}
