//! Tool result size limits.

/// Default maximum size in characters for tool results before they get
/// persisted to disk. When exceeded, the result is saved to a file and the
/// model receives a preview with the file path instead of the full content.
pub const DEFAULT_MAX_RESULT_SIZE_CHARS: usize = 50_000;

/// Maximum size for tool results in tokens.
/// Approximately 400 KB of text (assuming ~4 bytes per token).
pub const MAX_TOOL_RESULT_TOKENS: usize = 100_000;

/// Bytes-per-token estimate for calculating token count from byte size.
/// Conservative estimate -- actual token count may vary.
pub const BYTES_PER_TOKEN: usize = 4;

/// Maximum size for tool results in bytes (derived from token limit).
pub const MAX_TOOL_RESULT_BYTES: usize = MAX_TOOL_RESULT_TOKENS * BYTES_PER_TOKEN;

/// Default maximum aggregate size in characters for tool_result blocks within
/// a SINGLE user message (one turn's batch of parallel tool results).
/// Messages are evaluated independently.
pub const MAX_TOOL_RESULTS_PER_MESSAGE_CHARS: usize = 200_000;

/// Maximum character length for tool summary strings in compact views.
pub const TOOL_SUMMARY_MAX_LENGTH: usize = 50;
