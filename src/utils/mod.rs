//! ThunderCode shared utilities.
//!
//! This crate provides common utility functions used across the ThunderCode
//! workspace, ported from the TypeScript reference implementation.
//!
//! # Modules
//!
//! - [`format`] -- Number formatting, file sizes, durations, pluralization
//! - [`truncate`] -- Width-aware text truncation and wrapping
//! - [`json`] -- Safe JSON parsing, NDJSON/JSONL, partial JSON repair
//! - [`errors`] -- Common error types and classification
//! - [`memoize`] -- TTL and LRU memoization caches
//! - [`array`] -- Collection helpers (count, unique, chunk, reservoir sampling)
//! - [`paths`] -- Path expansion, relative conversion, project root detection
//! - [`sleep`] -- Async sleep with cancellation and timeout racing
//! - [`env`] -- Environment variable reading and classification

pub mod format;
pub mod truncate;
pub mod json;
pub mod errors;
pub mod memoize;
pub mod array;
pub mod paths;
pub mod sleep;
pub mod env;

// Re-export the most commonly used items at the crate root.
pub use errors::ThunderCodeError;
pub use format::{format_duration, format_file_size, format_tokens, pluralize, DurationOptions};
pub use json::{parse_jsonl, safe_parse_json};
pub use memoize::{MemoizeWithLru, MemoizeWithTtl, MemoizeWithTtlAsync};
pub use paths::{expand_path, to_relative_path};
pub use sleep::{sleep, with_timeout};
pub use truncate::{truncate, truncate_path_middle, truncate_to_width};
pub use env::{get_env, get_env_bool, is_env_truthy};
