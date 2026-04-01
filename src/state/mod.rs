//! ThunderCode reactive state management.
//!
//! This crate provides:
//!
//! - [`store::Store`] -- A generic, thread-safe reactive store with
//!   subscription support and same-value skip semantics.
//! - [`app_state::AppState`] -- The central UI application state.
//! - [`bootstrap::BootstrapState`] -- Process-scoped singleton state for
//!   execution context, cost tracking, and session metadata.
//! - [`notification::Notification`] -- Transient UI notification types.

pub mod store;
pub mod app_state;
pub mod bootstrap;
pub mod notification;

// Re-export the most commonly used items at crate root.
pub use app_state::{AppState, AppStateStore};
pub use bootstrap::{BootstrapState, BootstrapStateInner, ModelUsage};
pub use notification::{Notification, NotificationPriority};
pub use store::Store;
