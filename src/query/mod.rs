//! ThunderCode query engine -- the core inference loop.
//!
//! This crate owns the query lifecycle: it drives the model, dispatches
//! tool calls, handles compaction, manages error recovery, and enforces
//! token budgets. It is the heart of the system.
//!
//! # Architecture
//!
//! - **`engine`** -- `QueryEngine`, the top-level conversation owner.
//!   One engine per conversation. Each `submit_message()` starts a turn.
//!
//! - **`query`** -- The async inference loop. Produces a `Stream<QueryEvent>`.
//!   Handles stop reasons (`end_turn`, `tool_use`, `max_tokens`,
//!   `stop_sequence`), error recovery, and budget tracking.
//!
//! - **`config`** -- `QueryConfig` (immutable, snapshotted once) and
//!   `QueryDeps` (dependency injection for testability).
//!
//! - **`token_budget`** -- `BudgetTracker` with continuation/diminishing-
//!   returns logic, ported from `tokenBudget.ts`.
//!
//! - **`compaction`** -- Auto-compact and micro-compact stubs. Will be
//!   wired to `thundercode-services` when that crate exposes the compaction
//!   service.
//!
//! - **`stop_hooks`** -- Post-turn hook execution stubs.

pub mod compaction;
pub mod config;
pub mod engine;
pub mod query;
pub mod stop_hooks;
pub mod token_budget;

// Re-export the most commonly used types.
pub use config::{QueryConfig, QueryDeps, QueryGates};
pub use engine::{QueryEngine, QueryEngineBuilder};
pub use query::{ContinueReason, QueryEvent, QueryState, StopReason};
pub use token_budget::{BudgetDecision, BudgetTracker};
