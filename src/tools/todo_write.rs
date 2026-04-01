//! TodoWriteTool -- manage the session task checklist.
//!
//! Ported from ref/tools/TodoWriteTool/TodoWriteTool.ts.
//! Maintains a per-session todo list stored in memory (via AppState) and
//! optionally persisted to disk. The model uses this to track progress
//! on multi-step tasks.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};

pub const TODO_WRITE_TOOL_NAME: &str = "TodoWrite";

/// A single todo item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

/// Status of a todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// In-memory todo store. In a full implementation this would be part of
/// AppState. For now we use a global mutex so multiple tool calls within
/// a session share the same list.
static TODO_STORE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Vec<TodoItem>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Get the todo key for a given context (agent_id or "main").
fn todo_key(context: &ToolUseContext) -> String {
    context
        .agent_id
        .as_ref()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "main".to_string())
}

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        TODO_WRITE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("manage the session task checklist")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string", "description": "Imperative form: what needs to be done" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"]
                            },
                            "activeForm": {
                                "type": "string",
                                "description": "Present continuous form shown during execution (e.g., 'Running tests')"
                            }
                        },
                        "required": ["id", "content", "status"]
                    },
                    "description": "The updated todo list"
                }
            },
            "required": ["todos"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let key = todo_key(context);

        // Parse the incoming todos
        let todos_raw = input
            .get("todos")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut new_todos: Vec<TodoItem> = Vec::with_capacity(todos_raw.len());
        for item in &todos_raw {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status_str = item
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending");
            let status = match status_str {
                "in_progress" => TodoStatus::InProgress,
                "completed" => TodoStatus::Completed,
                _ => TodoStatus::Pending,
            };
            let active_form = item
                .get("activeForm")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            new_todos.push(TodoItem {
                id,
                content,
                status,
                active_form,
            });
        }

        // If all items are completed, clear the list (matches ref behavior)
        let all_done = new_todos.iter().all(|t| t.status == TodoStatus::Completed);
        let stored_todos = if all_done {
            Vec::new()
        } else {
            new_todos.clone()
        };

        // Swap old for new in the global store
        let old_todos = {
            let mut store = TODO_STORE.lock().unwrap();
            let old = store.get(&key).cloned().unwrap_or_default();
            store.insert(key.clone(), stored_todos);
            old
        };

        // Serialize old and new for the result
        let old_json: Vec<serde_json::Value> = old_todos
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "content": t.content,
                    "status": match t.status {
                        TodoStatus::Pending => "pending",
                        TodoStatus::InProgress => "in_progress",
                        TodoStatus::Completed => "completed",
                    }
                })
            })
            .collect();

        // Check if verification nudge is needed (3+ tasks all completed,
        // none of them mention verification)
        let verification_nudge_needed = context.agent_id.is_none()
            && all_done
            && new_todos.len() >= 3
            && !new_todos
                .iter()
                .any(|t| t.content.to_lowercase().contains("verif"));

        Ok(ToolCallResult {
            data: serde_json::json!({
                "oldTodos": old_json,
                "newTodos": todos_raw,
                "verificationNudgeNeeded": verification_nudge_needed,
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
        "Update the todo list for the current session. To be used proactively and often to track \
         progress and pending tasks. Make sure that at least one task is in_progress at all times. \
         Always provide both content (imperative) and activeForm (present continuous) for each task."
            .to_string()
    }

    async fn prompt(&self) -> String {
        include_str!("prompts/todo_write.txt").to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let count = input
            .get("todos")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        serde_json::Value::String(format!("{count} items"))
    }
}
