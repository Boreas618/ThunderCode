//! MonitorMcpTask -- long-running MCP monitor scripts.
//!
//! Ported from ref/tasks/MonitorMcpTask/MonitorMcpTask.ts.
//!
//! Monitor tasks run shell scripts that stream output continuously (e.g.,
//! tailing logs, watching file changes).  They are similar to shell tasks
//! but use a distinct task type so the UI can display them differently
//! (description-as-label instead of command, "Monitor details" dialog
//! title, distinct status bar pill).
//!
//! Gated behind the `MONITOR_TOOL` feature flag in the TypeScript reference.
//!
//! This is a stub; the full implementation reuses `ShellTask` internals
//! and will be completed when the monitor tool is ported.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// MonitorMcpTask
// ---------------------------------------------------------------------------

/// Handle for a monitor MCP task.
pub struct MonitorMcpTask {
    task_id: String,
    description: String,
    status: TaskStatus,
    /// The shell command being monitored.
    command: String,
}

impl MonitorMcpTask {
    pub fn new(task_id: String, description: String, command: String) -> Self {
        Self {
            task_id,
            description,
            status: TaskStatus::Pending,
            command,
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
        // TODO: kill the monitor process.
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

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn task_type() -> TaskType {
        TaskType::MonitorMcp
    }
}
