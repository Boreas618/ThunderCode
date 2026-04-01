//! LocalWorkflowTask -- user-defined workflow scripts.
//!
//! Ported from ref/tasks/LocalWorkflowTask/LocalWorkflowTask.ts.
//!
//! Workflow tasks execute user-defined scripts that orchestrate multiple
//! steps (e.g., test suites, deploy pipelines).  They are gated behind a
//! feature flag in the TypeScript reference (`WORKFLOW_SCRIPTS`).
//!
//! This is a stub; the full implementation will be added when the workflow
//! engine from `thundercode-services` is ported.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// LocalWorkflowTask
// ---------------------------------------------------------------------------

/// Handle for a local workflow task.
pub struct LocalWorkflowTask {
    task_id: String,
    description: String,
    status: TaskStatus,
    /// Path to the workflow script.
    script_path: String,
}

impl LocalWorkflowTask {
    pub fn new(task_id: String, description: String, script_path: String) -> Self {
        Self {
            task_id,
            description,
            status: TaskStatus::Pending,
            script_path,
        }
    }

    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
    }

    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = TaskStatus::Failed;
    }

    pub async fn kill(&mut self) {
        // TODO: kill the workflow script process.
        self.status = TaskStatus::Killed;
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn status(&self) -> TaskStatus {
        self.status
    }

    pub fn script_path(&self) -> &str {
        &self.script_path
    }

    pub fn task_type() -> TaskType {
        TaskType::LocalWorkflow
    }
}
