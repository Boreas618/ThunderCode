//! RemoteAgentTask -- a remote agent running on another machine / API.
//!
//! Ported from ref/tasks/RemoteAgentTask/RemoteAgentTask.tsx.
//!
//! Remote agent tasks communicate over the bridge WebSocket.  The full
//! implementation requires the bridge and remote execution protocol from
//! `thundercode-remote`.  This module provides the task-engine integration
//! point -- a handle that tracks status and can request cancellation.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// RemoteAgentTask
// ---------------------------------------------------------------------------

/// A handle to a remote agent task running on another machine.
pub struct RemoteAgentTask {
    task_id: String,
    description: String,
    status: TaskStatus,
    /// Remote endpoint URL or bridge connection ID.
    remote_endpoint: String,
}

impl RemoteAgentTask {
    pub fn new(task_id: String, description: String, remote_endpoint: String) -> Self {
        Self {
            task_id,
            description,
            status: TaskStatus::Pending,
            remote_endpoint,
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

    /// Request cancellation of the remote task.
    pub async fn kill(&mut self) {
        // TODO: send a cancellation request over the bridge.
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

    pub fn remote_endpoint(&self) -> &str {
        &self.remote_endpoint
    }

    pub fn task_type() -> TaskType {
        TaskType::RemoteAgent
    }
}
