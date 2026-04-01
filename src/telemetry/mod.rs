//! ThunderCode analytics and telemetry.
//!
//! This crate provides cost tracking, event logging, performance metrics,
//! and PII scrubbing for the ThunderCode coding assistant.

pub mod cost_tracker;
pub mod cost_hook;
pub mod events;
pub mod metrics;
pub mod pii;

pub use cost_tracker::{CostEntry, CostSummary, CostTracker, ModelCosts};
pub use cost_hook::{format_cost_summary, format_exit_summary, CodeChanges};
pub use events::{log_event, TelemetryEvent};
pub use metrics::Stats;
pub use pii::{scrub_file_paths, scrub_pii};
