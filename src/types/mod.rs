//! ThunderCode core types, message model, and IDs.
//!
//! This is the foundational types crate -- all other crates depend on it.
//! It contains no runtime logic, only data structures, enums, traits,
//! and serde implementations ported from the TypeScript reference.

pub mod content;
pub mod messages;
pub mod tool;
pub mod task;
pub mod command;
pub mod ids;
pub mod permissions;
pub mod settings;
pub mod hooks;
pub mod logs;
pub mod plugin;

// Re-export the most commonly used types at the crate root.
pub use content::{ContentBlock, ContentBlockParam, ImageSource, ToolResultContent};
pub use ids::{AgentId, SessionId, TaskId, TeamId};
pub use messages::{AssistantMessage, Message, ProgressMessage, SystemMessage, UserMessage};
pub use permissions::{PermissionMode, PermissionResult};
pub use task::{TaskStatus, TaskType};
pub use tool::{Tool, ToolCallResult, ToolError, ToolProgress, ToolProgressData, ToolUseContext};
