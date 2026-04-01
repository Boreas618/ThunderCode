//! TaskCreateTool -- create a new task (v2 task management).
//!
//! Ported from ref/tools/TaskCreateTool/TaskCreateTool.ts.
//! Creates a task in the persistent task list. Each task has a subject,
//! description, status, and optional metadata.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TASK_CREATE_TOOL_NAME: &str = "TaskCreate";

/// Simple in-memory task store for v2 tasks.
/// In a full implementation, this would be backed by the thundercode-tasks crate.
use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskV2 {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

pub static TASK_STORE: LazyLock<Mutex<Vec<TaskV2>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn generate_task_id() -> String {
    format!("t_{}", nanoid::nanoid!(8, &nanoid::alphabet::SAFE.iter().cloned().collect::<Vec<_>>()))
}

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        TASK_CREATE_TOOL_NAME
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
        Some("create a task in the task list")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "A brief title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "What needs to be done"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown in spinner when in_progress (e.g., 'Running tests')"
                },
                "metadata": {
                    "type": "object",
                    "description": "Arbitrary metadata to attach to the task"
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let active_form = input
            .get("activeForm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let metadata = input.get("metadata").cloned();

        let task_id = generate_task_id();
        let now = chrono::Utc::now().to_rfc3339();

        let task = TaskV2 {
            id: task_id.clone(),
            subject: subject.clone(),
            description,
            status: "pending".to_string(),
            active_form,
            owner: None,
            blocks: Vec::new(),
            blocked_by: Vec::new(),
            metadata,
            created_at: now,
            updated_at: None,
        };

        // Persist to the store
        {
            let mut store = TASK_STORE.lock().unwrap();
            store.push(task);
        }

        Ok(ToolCallResult {
            data: serde_json::json!({
                "task": {
                    "id": task_id,
                    "subject": subject,
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

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        format!("Create task: {subject}")
    }

    async fn prompt(&self) -> String {
        "Create a new task in the task list. Use this to track work items during multi-step tasks.\n\n\
         Each task needs:\n\
         - subject: A brief title (e.g., \"Fix auth bug\")\n\
         - description: What needs to be done\n\
         - activeForm (optional): Present continuous form for spinner display (e.g., \"Fixing auth bug\")\n\
         \n\
         Tasks start in 'pending' status. Use TaskUpdate to change status to 'in_progress' or 'completed'."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TaskCreate".to_string()
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        serde_json::Value::String(subject.to_string())
    }
}
