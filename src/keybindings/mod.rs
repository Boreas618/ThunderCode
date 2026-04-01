//! ThunderCode keybinding system.
//!
//! This crate implements the full keybinding system ported from the TypeScript
//! reference. It provides:
//!
//! - **Contexts** ([`context::KeybindingContext`]) -- the UI contexts where bindings are active.
//! - **Actions** ([`actions::KeybindingAction`]) -- all 90+ actions that can be triggered.
//! - **Bindings** ([`bindings`]) -- key combo parsing, the default binding table, and
//!   JSON config loading.
//! - **Chords** ([`chord`]) -- multi-key chord sequence support (e.g., `ctrl+x ctrl+e`).
//! - **Resolver** ([`resolver::KeybindingResolver`]) -- the main resolution engine that
//!   maps key events to actions, with chord state and custom override support.
//!
//! # Quick start
//!
//! ```rust
//! use crate::keybindings::resolver::{KeybindingResolver, ResolveResult};
//! use crate::keybindings::bindings::KeyCombo;
//! use crate::keybindings::context::KeybindingContext;
//! use crate::keybindings::actions::KeybindingAction;
//!
//! let mut resolver = KeybindingResolver::new();
//! let result = resolver.resolve(
//!     &KeyCombo::ctrl("c"),
//!     &[KeybindingContext::Global],
//! );
//! assert_eq!(result, ResolveResult::Match(KeybindingAction::AppInterrupt));
//! ```

pub mod actions;
pub mod bindings;
pub mod chord;
pub mod context;
pub mod resolver;

// Re-export key types for convenience.
pub use actions::KeybindingAction;
pub use bindings::{KeyBinding, KeyCombo};
pub use context::KeybindingContext;
pub use resolver::{crossterm_key_to_combo, KeybindingResolver, ResolveResult};
