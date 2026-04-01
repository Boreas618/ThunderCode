//! ThunderCode tool implementations and registry.
//!
//! This crate contains the tool registry (`ToolRegistry`) and all 40+ tool
//! implementations that the LLM uses to interact with the system -- reading
//! files, running commands, searching, editing, spawning agents, etc.
//!
//! Ported from ref/tools.ts and ref/tools/*.

pub mod registry;

// Core file tools -- fully implemented
pub mod bash;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob_tool;
pub mod grep;

// Web tools
pub mod web_fetch;
pub mod web_search;

// Task tools
pub mod task_create;
pub mod task_get;
pub mod task_list;
pub mod task_output;
pub mod task_stop;
pub mod task_update;

// Planning tools
pub mod enter_plan_mode;
pub mod exit_plan_mode;

// Worktree tools
pub mod enter_worktree;
pub mod exit_worktree;

// Agent/collaboration tools
pub mod agent;
pub mod send_message;
pub mod skill;
pub mod team_create;
pub mod team_delete;

// Utility tools
pub mod ask_user;
pub mod brief;
pub mod cron_create;
pub mod cron_delete;
pub mod cron_list;
pub mod list_mcp_resources;
pub mod lsp;
pub mod mcp_tool;
pub mod notebook_edit;
pub mod read_mcp_resource;
pub mod sleep;
pub mod todo_write;
pub mod tool_search;

// Re-export registry and common types.
pub use registry::ToolRegistry;
