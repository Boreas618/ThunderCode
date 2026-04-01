//! LocalAgentTask -- a nested agent running in a background task.
//!
//! Ported from ref/tasks/LocalAgentTask/LocalAgentTask.tsx.
//!
//! The agent task spawns a child agent context that runs autonomously in the
//! background.  Communication with the parent happens through the task engine
//! and the shared `AppState`.
//!
//! This is a stub implementation.  The full agent execution loop lives in
//! `thundercode-session` and will be wired up when the agent runtime is ported.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// AgentTask
// ---------------------------------------------------------------------------

/// A background agent task.
///
/// In the TypeScript reference this holds the full agent context including
/// message history, abort controller, and progress tracking.  For now we
/// store only the metadata needed by the task engine.
pub struct AgentTask {
    task_id: String,
    description: String,
    status: TaskStatus,
    /// The agent prompt that started this task.
    prompt: String,
}

impl AgentTask {
    /// Create a new agent task (does not start execution).
    pub fn new(task_id: String, description: String, prompt: String) -> Self {
        Self {
            task_id,
            description,
            status: TaskStatus::Pending,
            prompt,
        }
    }

    /// Mark the task as running.
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
    }

    /// Mark the task as completed.
    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
    }

    /// Mark the task as failed.
    pub fn fail(&mut self) {
        self.status = TaskStatus::Failed;
    }

    /// Kill the agent task.
    pub async fn kill(&mut self) {
        // TODO: abort the agent's query loop via a cancellation token.
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

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn task_type() -> TaskType {
        TaskType::LocalAgent
    }
}
