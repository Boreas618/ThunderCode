//! ThunderCode system and user context management.
//!
//! This crate provides:
//!
//! - [`system_context::SystemContext`] -- Git status, branch, platform info
//!   gathered once per conversation.
//! - [`user_context::UserContext`] -- RULES.md content and date, gathered
//!   once per conversation.
//! - [`notifications::NotificationQueue`] -- Priority-based queue for
//!   transient TUI notifications.
//! - [`prompt_builder::SystemPromptBuilder`] -- Assembles the full system
//!   prompt from all context sources.

pub mod notifications;
pub mod prompt_builder;
pub mod system_context;
pub mod user_context;

// Re-export the primary public types at the crate root.
pub use notifications::NotificationQueue;
pub use prompt_builder::SystemPromptBuilder;
pub use system_context::{get_system_context, SystemContext};
pub use user_context::{get_user_context, RulesMdFile, UserContext};
