//! ThunderCode slash commands and command registry.
//!
//! This crate provides:
//!
//! - [`CommandRegistry`] -- The central registry that holds all built-in
//!   slash commands and supports lookup by name or alias.
//! - Individual command constructors organized by category (session, git,
//!   model, tools, settings, features, system, review).
//!
//! Each command is represented as a [`Command`] enum variant from
//! `crate::types::command`.  Commands are registered at startup and
//! looked up by the REPL when the user types `/command-name`.
//!
//! Ported from ref/commands.ts and ref/commands/*.

pub mod registry;
pub mod commands;

// Re-export the registry as the primary public API.
pub use registry::CommandRegistry;
