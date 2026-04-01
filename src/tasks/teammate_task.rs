//! InProcessTeammateTask -- a teammate agent running in the same process.
//!
//! Ported from ref/tasks/InProcessTeammateTask/InProcessTeammateTask.tsx.
//!
//! Teammates are in-process agents that share the same event loop as the
//! leader.  They have their own conversation history, permission mode, and
//! mailbox.  The full execution loop is wired through `thundercode-session`;
//! this module provides the task handle and lifecycle management.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// TeammateIdentity
// ---------------------------------------------------------------------------

/// Identity metadata for a teammate, stored in AppState.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeammateIdentity {
    pub agent_id: String,
    pub agent_name: String,
    pub team_name: String,
    pub color: Option<String>,
    pub plan_mode_required: bool,
    pub parent_session_id: String,
}

// ---------------------------------------------------------------------------
// InProcessTeammateTask
// ---------------------------------------------------------------------------

/// Handle for an in-process teammate task.
pub struct InProcessTeammateTask {
    task_id: String,
    description: String,
    status: TaskStatus,
    identity: TeammateIdentity,
    prompt: String,
    is_idle: bool,
    shutdown_requested: bool,
}

impl InProcessTeammateTask {
    pub fn new(
        task_id: String,
        description: String,
        identity: TeammateIdentity,
        prompt: String,
    ) -> Self {
        Self {
            task_id,
            description,
            status: TaskStatus::Pending,
            identity,
            prompt,
            is_idle: false,
            shutdown_requested: false,
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

    /// Request a graceful shutdown of the teammate.
    pub fn request_shutdown(&mut self) {
        self.shutdown_requested = true;
    }

    /// Kill the teammate task immediately.
    pub async fn kill(&mut self) {
        // TODO: abort the teammate's query loop.
        self.status = TaskStatus::Killed;
    }

    pub fn set_idle(&mut self, idle: bool) {
        self.is_idle = idle;
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

    pub fn identity(&self) -> &TeammateIdentity {
        &self.identity
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn is_idle(&self) -> bool {
        self.is_idle
    }

    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    pub fn task_type() -> TaskType {
        TaskType::InProcessTeammate
    }
}
