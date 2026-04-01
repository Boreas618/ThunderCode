//! ThunderCode services layer (API, MCP, compact, etc.).
//!
//! This crate provides high-level service abstractions on top of the raw API
//! client, types, and configuration layers. Ported from the TypeScript
//! `services/` directory in the reference implementation.

pub mod analytics;
pub mod api_service;
pub mod compact;
pub mod diagnostics;
pub mod notifier;
pub mod prevent_sleep;
pub mod voice;
