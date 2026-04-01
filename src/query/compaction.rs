//! Message compaction -- auto-compact and micro-compact.
//!
//! When the conversation grows too large for the model's context window,
//! compaction summarizes older messages to free up tokens while preserving
//! the essential information the model needs.

use crate::types::messages::Message;

use crate::query::config::QueryConfig;

// ---------------------------------------------------------------------------
// AutoCompactState
// ---------------------------------------------------------------------------

/// Mutable state carried across loop iterations for auto-compaction.
///
/// Tracks whether compaction has already been triggered, how many turns
/// have elapsed since the last compact, and consecutive failure counts
/// for circuit-breaking.
#[derive(Debug, Clone)]
pub struct AutoCompactState {
    /// Whether the conversation has been compacted at least once.
    pub compacted: bool,

    /// A unique identifier for the current compaction epoch (refreshed on
    /// each successful compact).
    pub turn_id: String,

    /// Number of turns since the last compaction. Incremented after each
    /// tool-use continuation; reset on compact.
    pub turn_counter: u32,

    /// Consecutive compaction failures. Used to circuit-break: after N
    /// failures in a row, stop trying until the user acts.
    pub consecutive_failures: u32,
}

impl Default for AutoCompactState {
    fn default() -> Self {
        Self {
            compacted: false,
            turn_id: String::new(),
            turn_counter: 0,
            consecutive_failures: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// CompactionResult
// ---------------------------------------------------------------------------

/// Result of a successful auto-compaction pass.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Summarized messages that replace the old conversation prefix.
    pub summary_messages: Vec<Message>,

    /// Attachment messages produced during compaction hooks.
    pub attachments: Vec<Message>,

    /// Messages produced by post-compact hooks.
    pub hook_results: Vec<Message>,

    /// Token counts before and after compaction, for analytics.
    pub pre_compact_token_count: u64,
    pub post_compact_token_count: u64,
    pub true_post_compact_token_count: Option<u64>,
}

// ---------------------------------------------------------------------------
// auto_compact
// ---------------------------------------------------------------------------

/// Run auto-compaction if the message history exceeds the threshold.
///
/// Returns `Ok(Some(result))` if compaction was performed, `Ok(None)` if
/// the conversation is still within budget, or `Err` on failure.
///
/// The compaction itself calls the model to produce a summary. In this
/// initial port, the function is a stub that always returns `Ok(None)`
/// (no compaction). The real implementation will be wired up when the
/// compaction service crate is available.
pub async fn auto_compact(
    _messages: &mut Vec<Message>,
    _state: &mut AutoCompactState,
    _config: &QueryConfig,
) -> anyhow::Result<Option<CompactionResult>> {
    // TODO: Wire up the compaction service once thundercode-services exposes it.
    // The TS implementation:
    // 1. Estimates token count from messages.
    // 2. If above threshold, calls the model with a compaction prompt.
    // 3. Replaces old messages with the compact boundary + summary.
    // 4. Returns CompactionResult with pre/post token counts.
    Ok(None)
}

// ---------------------------------------------------------------------------
// micro_compact
// ---------------------------------------------------------------------------

/// Apply micro-compaction to messages.
///
/// Micro-compact trims large tool results that the model is unlikely to
/// re-read, replacing them with a compact marker. Unlike full auto-compact,
/// this never calls the model -- it operates purely on content size heuristics.
///
/// In this initial port, the function is a no-op stub.
pub async fn micro_compact(_messages: &mut Vec<Message>) -> anyhow::Result<()> {
    // TODO: Port the heuristic trimming from microCompact.ts.
    // The TS implementation:
    // 1. Walks tool_result content blocks.
    // 2. If a result exceeds a size threshold and is not the most recent,
    //    replaces its content with "[truncated]".
    // 3. Caches the truncation state by tool_use_id.
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the post-compaction message array from a `CompactionResult`.
///
/// Returns a flat list: summary messages + attachments + hook results.
pub fn build_post_compact_messages(result: &CompactionResult) -> Vec<Message> {
    let mut out = Vec::with_capacity(
        result.summary_messages.len() + result.attachments.len() + result.hook_results.len(),
    );
    out.extend(result.summary_messages.iter().cloned());
    out.extend(result.attachments.iter().cloned());
    out.extend(result.hook_results.iter().cloned());
    out
}
