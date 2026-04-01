//! Conversation compaction service.
//!
//! When the conversation grows too large for the model's context window, the
//! compaction service summarises the older portion and replaces it with a
//! condensed summary. This module ports the core flow from
//! `ref/services/compact/` (compact.ts, autoCompact.ts, microCompact.ts,
//! sessionMemoryCompact.ts, prompt.ts).

mod auto_compact;
mod micro_compact;
mod prompt;
mod session_memory_compact;
mod summarize;

// Re-export the public API.
pub use auto_compact::{
    auto_compact_check, get_auto_compact_threshold, get_effective_context_window_size,
    is_auto_compact_enabled, AutoCompactTrackingState,
};
pub use micro_compact::{micro_compact, MicrocompactResult};
pub use prompt::{format_compact_summary, get_compact_prompt, get_compact_user_summary_message};
pub use session_memory_compact::{session_memory_compact, SessionMemoryCompactConfig};
pub use summarize::{compact_messages, CompactionResult};

/// Buffer tokens subtracted from the effective context window for autocompact.
pub const AUTOCOMPACT_BUFFER_TOKENS: u64 = 13_000;

/// Buffer before the warning threshold is hit.
pub const WARNING_THRESHOLD_BUFFER_TOKENS: u64 = 20_000;

/// Buffer before the error threshold is hit.
pub const ERROR_THRESHOLD_BUFFER_TOKENS: u64 = 20_000;

/// Buffer before the blocking limit for manual compact.
pub const MANUAL_COMPACT_BUFFER_TOKENS: u64 = 3_000;

/// Maximum output tokens reserved for the summary during compaction.
/// Based on p99.99 of compact summary output being 17,387 tokens.
pub const MAX_OUTPUT_TOKENS_FOR_SUMMARY: u32 = 20_000;

/// Stop retrying autocompact after this many consecutive failures.
pub const MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES: u32 = 3;
