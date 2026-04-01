//! ThunderCode plugin system.
//!
//! This crate handles:
//! - Plugin registry for tracking loaded and built-in plugins ([`registry`])
//! - Loading plugin manifests from the filesystem ([`loader`])
//! - Plugin lifecycle management ([`lifecycle`])
//! - Built-in plugin definitions ([`builtin`])
//! - Plugin marketplace client ([`marketplace`])
//!
//! Ported from ref/plugins/builtinPlugins.ts` and `ref/plugins/bundled/`.

pub mod builtin;
pub mod lifecycle;
pub mod loader;
pub mod marketplace;
pub mod registry;

// Re-export the most commonly used types at the crate root.
pub use builtin::get_builtin_plugins;
pub use lifecycle::PluginLifecycle;
pub use loader::{load_plugin_from_path, load_plugins_from_directory};
pub use marketplace::MarketplaceClient;
pub use registry::PluginRegistry;
