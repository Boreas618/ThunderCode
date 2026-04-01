//! Task types for background task management.
//!
//! Ported from ref/Task.ts.

use serde::{Deserialize, Serialize};

// ============================================================================
// TaskType
// ============================================================================

/// The kind of background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    LocalBash,
    LocalAgent,
    RemoteAgent,
    InProcessTeammate,
    LocalWorkflow,
    MonitorMcp,
    Dream,
}

impl TaskType {
    /// Get the single-character prefix for task ID generation.
    pub fn id_prefix(&self) -> char {
        match self {
            TaskType::LocalBash => 'b',
            TaskType::LocalAgent => 'a',
            TaskType::RemoteAgent => 'r',
            TaskType::InProcessTeammate => 't',
            TaskType::LocalWorkflow => 'w',
            TaskType::MonitorMcp => 'm',
            TaskType::Dream => 'd',
        }
    }
}

// ============================================================================
// TaskStatus
// ============================================================================

/// The current status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

impl TaskStatus {
    /// True when a task is in a terminal state and will not transition further.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Killed)
    }
}

// ============================================================================
// TaskStateBase
// ============================================================================

/// Base fields shared by all task states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskStateBase {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    /// Epoch milliseconds.
    pub start_time: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_paused_ms: Option<u64>,
    pub output_file: String,
    pub output_offset: usize,
    pub notified: bool,
}

// ============================================================================
// TaskHandle
// ============================================================================

/// A lightweight handle to a running task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHandle {
    pub task_id: String,
}

// ============================================================================
// LocalShellSpawnInput
// ============================================================================

/// Input for spawning a local shell task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalShellSpawnInput {
    pub command: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<crate::types::ids::AgentId>,
    /// UI display variant: description-as-label, dialog title, status bar pill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<TaskDisplayKind>,
}

/// Display kind for a task in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskDisplayKind {
    Bash,
    Monitor,
}

// ============================================================================
// Task ID Generation
// ============================================================================

/// Case-insensitive-safe alphabet (digits + lowercase) for task IDs.
/// 36^8 ~ 2.8 trillion combinations.
const TASK_ID_ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Generate a task ID with a type-specific prefix + 8 random alphanumeric characters.
pub fn generate_task_id(task_type: TaskType) -> String {
    let prefix = task_type.id_prefix();
    let alphabet: Vec<char> = TASK_ID_ALPHABET.iter().map(|&b| b as char).collect();
    let suffix = nanoid::nanoid!(8, &alphabet);
    format!("{}{}", prefix, suffix)
}

/// Create an initial `TaskStateBase` in `Pending` status.
pub fn create_task_state_base(
    id: String,
    task_type: TaskType,
    description: String,
    tool_use_id: Option<String>,
    output_file: String,
) -> TaskStateBase {
    TaskStateBase {
        id,
        task_type,
        status: TaskStatus::Pending,
        description,
        tool_use_id,
        start_time: chrono::Utc::now().timestamp_millis() as u64,
        end_time: None,
        total_paused_ms: None,
        output_file,
        output_offset: 0,
        notified: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_statuses() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Killed.is_terminal());
    }

    #[test]
    fn task_id_format() {
        let id = generate_task_id(TaskType::LocalBash);
        assert!(id.starts_with('b'));
        assert_eq!(id.len(), 9); // 1 prefix + 8 random
    }

    #[test]
    fn task_id_prefixes() {
        assert_eq!(TaskType::LocalBash.id_prefix(), 'b');
        assert_eq!(TaskType::LocalAgent.id_prefix(), 'a');
        assert_eq!(TaskType::RemoteAgent.id_prefix(), 'r');
        assert_eq!(TaskType::InProcessTeammate.id_prefix(), 't');
        assert_eq!(TaskType::LocalWorkflow.id_prefix(), 'w');
        assert_eq!(TaskType::MonitorMcp.id_prefix(), 'm');
        assert_eq!(TaskType::Dream.id_prefix(), 'd');
    }
}
