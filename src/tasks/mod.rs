//! ThunderCode task engine -- background task spawning, lifecycle, and output.
//!
//! This crate provides:
//!
//! - [`engine::TaskEngine`] -- Central registry for all background tasks.
//! - [`shell_task::ShellTask`] -- Async subprocess management via `tokio::process`.
//! - [`agent_task::AgentTask`] -- Nested agent tasks (stub).
//! - [`remote_task::RemoteAgentTask`] -- Remote agent tasks (stub).
//! - [`teammate_task::InProcessTeammateTask`] -- In-process teammate tasks (stub).
//! - [`dream_task::DreamTask`] -- Memory consolidation tasks.
//! - [`workflow_task::LocalWorkflowTask`] -- User-defined workflow scripts (stub).
//! - [`monitor_task::MonitorMcpTask`] -- Long-running MCP monitors (stub).
//! - [`output`] -- Disk-based task output file management.
//!
//! Ported from the TypeScript reference in ref/Task.ts, ref/tasks.ts, and
//! the individual task implementations under ref/tasks/.

pub mod engine;
pub mod output;
pub mod shell_task;

// Task type modules.
pub mod agent_task;
pub mod dream_task;
pub mod monitor_task;
pub mod remote_task;
pub mod teammate_task;
pub mod workflow_task;

// Re-export the most commonly used items.
pub use engine::{TaskEngine, TaskHandle, TaskInput};
pub use output::{
    append_task_output, cleanup_task_output, ensure_task_output_dir, get_task_output_path,
    get_task_output_size, init_task_output, read_task_output,
};
pub use shell_task::ShellTask;
