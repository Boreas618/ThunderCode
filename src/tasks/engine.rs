//! Task engine -- central registry and lifecycle manager for all background tasks.
//!
//! Ported from the task management patterns in ref/Task.ts, ref/tasks.ts,
//! and ref/tasks/stopTask.ts.
//!
//! The engine owns every running task and provides a uniform interface to
//! spawn, kill, query, and list them.  It is the single source of truth for
//! task status within the process.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{bail, Result};
use crate::types::task::{self, TaskStatus, TaskType};
use tracing;

use crate::tasks::output;
use crate::tasks::shell_task::ShellTask;

// ---------------------------------------------------------------------------
// TaskInput -- polymorphic spawn input
// ---------------------------------------------------------------------------

/// Input required to spawn a task.  Variants mirror the different task types.
#[derive(Debug, Clone)]
pub enum TaskInput {
    /// Spawn a local shell command.
    Shell {
        command: String,
        description: String,
        timeout: Option<Duration>,
    },
    /// Spawn a local agent task.
    Agent {
        description: String,
        prompt: String,
    },
    /// Spawn a remote agent task.
    RemoteAgent {
        description: String,
        remote_endpoint: String,
    },
    /// Spawn an in-process teammate.
    Teammate {
        description: String,
        prompt: String,
    },
    /// Spawn a dream (memory consolidation) task.
    Dream {
        sessions_reviewing: usize,
    },
    /// Spawn a local workflow task.
    Workflow {
        description: String,
        script_path: String,
    },
    /// Spawn a monitor MCP task.
    Monitor {
        description: String,
        command: String,
    },
}

impl TaskInput {
    /// Derive the `TaskType` from the input variant.
    pub fn task_type(&self) -> TaskType {
        match self {
            TaskInput::Shell { .. } => TaskType::LocalBash,
            TaskInput::Agent { .. } => TaskType::LocalAgent,
            TaskInput::RemoteAgent { .. } => TaskType::RemoteAgent,
            TaskInput::Teammate { .. } => TaskType::InProcessTeammate,
            TaskInput::Dream { .. } => TaskType::Dream,
            TaskInput::Workflow { .. } => TaskType::LocalWorkflow,
            TaskInput::Monitor { .. } => TaskType::MonitorMcp,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskHandle -- type-erased wrapper around concrete task types
// ---------------------------------------------------------------------------

/// A type-erased handle to a running task.  Each variant holds the concrete
/// task struct for its type.
pub enum TaskHandle {
    Shell(ShellTask),
    Agent(crate::tasks::agent_task::AgentTask),
    RemoteAgent(crate::tasks::remote_task::RemoteAgentTask),
    Teammate(crate::tasks::teammate_task::InProcessTeammateTask),
    Dream(crate::tasks::dream_task::DreamTask),
    Workflow(crate::tasks::workflow_task::LocalWorkflowTask),
    Monitor(crate::tasks::monitor_task::MonitorMcpTask),
}

impl TaskHandle {
    /// Return the current status of the wrapped task.
    pub fn status(&self) -> TaskStatus {
        match self {
            TaskHandle::Shell(_) => {
                // ShellTask doesn't track status internally -- the engine
                // keeps a parallel status field.  We return Running as a
                // default; the engine overwrites this.
                TaskStatus::Running
            }
            TaskHandle::Agent(t) => t.status(),
            TaskHandle::RemoteAgent(t) => t.status(),
            TaskHandle::Teammate(t) => t.status(),
            TaskHandle::Dream(t) => t.status(),
            TaskHandle::Workflow(t) => t.status(),
            TaskHandle::Monitor(t) => t.status(),
        }
    }

    /// Return the task type of the wrapped task.
    pub fn task_type(&self) -> TaskType {
        match self {
            TaskHandle::Shell(_) => TaskType::LocalBash,
            TaskHandle::Agent(_) => TaskType::LocalAgent,
            TaskHandle::RemoteAgent(_) => TaskType::RemoteAgent,
            TaskHandle::Teammate(_) => TaskType::InProcessTeammate,
            TaskHandle::Dream(_) => TaskType::Dream,
            TaskHandle::Workflow(_) => TaskType::LocalWorkflow,
            TaskHandle::Monitor(_) => TaskType::MonitorMcp,
        }
    }

    /// Return the description of the wrapped task.
    pub fn description(&self) -> &str {
        match self {
            TaskHandle::Shell(t) => t.description(),
            TaskHandle::Agent(t) => t.description(),
            TaskHandle::RemoteAgent(t) => t.description(),
            TaskHandle::Teammate(t) => t.description(),
            TaskHandle::Dream(_) => "dreaming",
            TaskHandle::Workflow(t) => t.description(),
            TaskHandle::Monitor(t) => t.description(),
        }
    }

    /// Kill the wrapped task.
    async fn kill(&mut self) -> Result<()> {
        match self {
            TaskHandle::Shell(t) => t.kill().await,
            TaskHandle::Agent(t) => {
                t.kill().await;
                Ok(())
            }
            TaskHandle::RemoteAgent(t) => {
                t.kill().await;
                Ok(())
            }
            TaskHandle::Teammate(t) => {
                t.kill().await;
                Ok(())
            }
            TaskHandle::Dream(t) => {
                t.kill().await;
                Ok(())
            }
            TaskHandle::Workflow(t) => {
                t.kill().await;
                Ok(())
            }
            TaskHandle::Monitor(t) => {
                t.kill().await;
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TaskEntry -- metadata + handle stored in the engine
// ---------------------------------------------------------------------------

/// Internal bookkeeping for a single task.
#[allow(dead_code)]
struct TaskEntry {
    handle: TaskHandle,
    status: TaskStatus,
    task_type: TaskType,
    description: String,
    start_time: u64,
    end_time: Option<u64>,
}

// ---------------------------------------------------------------------------
// TaskEngine
// ---------------------------------------------------------------------------

/// Central registry and lifecycle manager for all background tasks.
///
/// # Usage
///
/// ```rust,ignore
/// let mut engine = TaskEngine::new();
/// let id = engine.spawn(TaskInput::Shell {
///     command: "cargo test".into(),
///     description: "run tests".into(),
///     timeout: None,
/// }).await?;
///
/// // Later...
/// engine.kill(&id).await?;
/// ```
pub struct TaskEngine {
    tasks: HashMap<String, TaskEntry>,
}

impl TaskEngine {
    /// Create a new, empty task engine.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Spawn a new task and return its generated task ID.
    pub async fn spawn(&mut self, input: TaskInput) -> Result<String> {
        let task_type = input.task_type();
        let task_id = task::generate_task_id(task_type);
        let now = chrono::Utc::now().timestamp_millis() as u64;

        tracing::info!(
            task_id = %task_id,
            task_type = ?task_type,
            "spawning task"
        );

        let (handle, description) = match input {
            TaskInput::Shell {
                command,
                description,
                timeout,
            } => {
                let shell = ShellTask::spawn(&task_id, &command, &description, timeout).await?;
                let desc = description.clone();
                (TaskHandle::Shell(shell), desc)
            }
            TaskInput::Agent {
                description,
                prompt,
            } => {
                let mut agent =
                    crate::tasks::agent_task::AgentTask::new(task_id.clone(), description.clone(), prompt);
                agent.start();
                (TaskHandle::Agent(agent), description)
            }
            TaskInput::RemoteAgent {
                description,
                remote_endpoint,
            } => {
                let mut remote = crate::tasks::remote_task::RemoteAgentTask::new(
                    task_id.clone(),
                    description.clone(),
                    remote_endpoint,
                );
                remote.start();
                (TaskHandle::RemoteAgent(remote), description)
            }
            TaskInput::Teammate {
                description,
                prompt,
            } => {
                // Minimal identity for now -- the full wiring comes from the
                // session/team module.
                let identity = crate::tasks::teammate_task::TeammateIdentity {
                    agent_id: task_id.clone(),
                    agent_name: "teammate".into(),
                    team_name: "default".into(),
                    color: None,
                    plan_mode_required: false,
                    parent_session_id: String::new(),
                };
                let mut teammate = crate::tasks::teammate_task::InProcessTeammateTask::new(
                    task_id.clone(),
                    description.clone(),
                    identity,
                    prompt,
                );
                teammate.start();
                (TaskHandle::Teammate(teammate), description)
            }
            TaskInput::Dream {
                sessions_reviewing,
            } => {
                let dream =
                    crate::tasks::dream_task::DreamTask::new(task_id.clone(), sessions_reviewing);
                (TaskHandle::Dream(dream), "dreaming".into())
            }
            TaskInput::Workflow {
                description,
                script_path,
            } => {
                let mut wf = crate::tasks::workflow_task::LocalWorkflowTask::new(
                    task_id.clone(),
                    description.clone(),
                    script_path,
                );
                wf.start();
                (TaskHandle::Workflow(wf), description)
            }
            TaskInput::Monitor {
                description,
                command,
            } => {
                let mut mon = crate::tasks::monitor_task::MonitorMcpTask::new(
                    task_id.clone(),
                    description.clone(),
                    command,
                );
                mon.start();
                (TaskHandle::Monitor(mon), description)
            }
        };

        self.tasks.insert(
            task_id.clone(),
            TaskEntry {
                handle,
                status: TaskStatus::Running,
                task_type,
                description,
                start_time: now,
                end_time: None,
            },
        );

        Ok(task_id)
    }

    /// Kill a running task by ID.
    ///
    /// Returns an error if the task does not exist.  If the task is already
    /// in a terminal state, this is a no-op.
    pub async fn kill(&mut self, task_id: &str) -> Result<()> {
        let entry = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("no task found with ID: {}", task_id))?;

        if entry.status.is_terminal() {
            tracing::debug!(task_id = task_id, "task already in terminal state, skipping kill");
            return Ok(());
        }

        tracing::info!(task_id = task_id, "killing task");

        entry.handle.kill().await?;
        entry.status = TaskStatus::Killed;
        entry.end_time = Some(chrono::Utc::now().timestamp_millis() as u64);

        // Best-effort cleanup of the output file.
        output::cleanup_task_output(task_id).await;

        Ok(())
    }

    /// Get the current status of a task, or `None` if the ID is unknown.
    pub fn get_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.tasks.get(task_id).map(|e| e.status)
    }

    /// Get the task type for a task, or `None` if the ID is unknown.
    pub fn get_task_type(&self, task_id: &str) -> Option<TaskType> {
        self.tasks.get(task_id).map(|e| e.task_type)
    }

    /// Get the description for a task, or `None` if the ID is unknown.
    pub fn get_description(&self, task_id: &str) -> Option<&str> {
        self.tasks.get(task_id).map(|e| e.description.as_str())
    }

    /// List all tasks with their current status.
    pub fn list_tasks(&self) -> Vec<(String, TaskStatus)> {
        self.tasks
            .iter()
            .map(|(id, entry)| (id.clone(), entry.status))
            .collect()
    }

    /// List only running tasks.
    pub fn list_running(&self) -> Vec<(String, TaskType, &str)> {
        self.tasks
            .iter()
            .filter(|(_, e)| e.status == TaskStatus::Running)
            .map(|(id, e)| (id.clone(), e.task_type, e.description.as_str()))
            .collect()
    }

    /// Update the status of a task (used by external completion handlers).
    ///
    /// Returns `false` if the task does not exist.
    pub fn set_status(&mut self, task_id: &str, status: TaskStatus) -> bool {
        if let Some(entry) = self.tasks.get_mut(task_id) {
            entry.status = status;
            if status.is_terminal() && entry.end_time.is_none() {
                entry.end_time = Some(chrono::Utc::now().timestamp_millis() as u64);
            }
            true
        } else {
            false
        }
    }

    /// Get a reference to the task handle (for advanced callers that need
    /// direct access to the concrete task type).
    pub fn get_handle(&self, task_id: &str) -> Option<&TaskHandle> {
        self.tasks.get(task_id).map(|e| &e.handle)
    }

    /// Get a mutable reference to the task handle.
    pub fn get_handle_mut(&mut self, task_id: &str) -> Option<&mut TaskHandle> {
        self.tasks.get_mut(task_id).map(|e| &mut e.handle)
    }

    /// Remove all tasks in terminal states from the engine.
    ///
    /// Returns the number of tasks removed.
    pub fn evict_completed(&mut self) -> usize {
        let before = self.tasks.len();
        self.tasks.retain(|_, e| !e.status.is_terminal());
        before - self.tasks.len()
    }

    /// Kill all running tasks.  Used during shutdown.
    pub async fn kill_all(&mut self) -> Result<()> {
        let running_ids: Vec<String> = self
            .tasks
            .iter()
            .filter(|(_, e)| !e.status.is_terminal())
            .map(|(id, _)| id.clone())
            .collect();

        let mut errors = Vec::new();
        for id in running_ids {
            if let Err(e) = self.kill(&id).await {
                tracing::warn!(task_id = %id, error = %e, "failed to kill task during shutdown");
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            bail!(
                "failed to kill {} task(s) during shutdown",
                errors.len()
            )
        }
    }

    /// Return how many tasks are currently tracked (all states).
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Return true if no tasks are tracked.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl Default for TaskEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_shell_and_list() {
        let mut engine = TaskEngine::new();
        let id = engine
            .spawn(TaskInput::Shell {
                command: "echo hi".into(),
                description: "test".into(),
                timeout: None,
            })
            .await
            .unwrap();

        assert_eq!(engine.get_status(&id), Some(TaskStatus::Running));
        assert!(!engine.list_tasks().is_empty());

        // Cleanup.
        let _ = engine.kill(&id).await;
        output::cleanup_task_output(&id).await;
    }

    #[tokio::test]
    async fn kill_sets_terminal_status() {
        let mut engine = TaskEngine::new();
        let id = engine
            .spawn(TaskInput::Shell {
                command: "sleep 60".into(),
                description: "long task".into(),
                timeout: None,
            })
            .await
            .unwrap();

        engine.kill(&id).await.unwrap();
        assert_eq!(engine.get_status(&id), Some(TaskStatus::Killed));

        // Killing again is a no-op.
        engine.kill(&id).await.unwrap();

        output::cleanup_task_output(&id).await;
    }

    #[tokio::test]
    async fn unknown_task_returns_none() {
        let engine = TaskEngine::new();
        assert_eq!(engine.get_status("nonexistent"), None);
    }

    #[tokio::test]
    async fn evict_completed() {
        let mut engine = TaskEngine::new();
        let id = engine
            .spawn(TaskInput::Agent {
                description: "test agent".into(),
                prompt: "do something".into(),
            })
            .await
            .unwrap();

        engine.set_status(&id, TaskStatus::Completed);
        assert_eq!(engine.evict_completed(), 1);
        assert!(engine.is_empty());
    }

    #[test]
    fn default_engine_is_empty() {
        let engine = TaskEngine::new();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);
    }
}
