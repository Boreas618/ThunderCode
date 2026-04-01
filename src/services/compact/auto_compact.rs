//! Auto-compaction: token-threshold monitoring and automatic compaction triggers.
//!
//! Ported from ref/services/compact/autoCompact.ts`. Monitors the conversation
//! token count against the model's effective context window and triggers
//! compaction when the threshold is exceeded.

use crate::api::models::get_model_info;
use crate::types::Message;

use super::summarize::estimate_message_tokens;
use super::{
    AUTOCOMPACT_BUFFER_TOKENS, MANUAL_COMPACT_BUFFER_TOKENS, MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES,
    MAX_OUTPUT_TOKENS_FOR_SUMMARY,
};

// ---------------------------------------------------------------------------
// AutoCompactTrackingState
// ---------------------------------------------------------------------------

/// Per-session tracking state for auto-compaction.
#[derive(Debug, Clone)]
pub struct AutoCompactTrackingState {
    /// Whether compaction has occurred this session.
    pub compacted: bool,

    /// Number of turns since the last compaction (or session start).
    pub turn_counter: u32,

    /// Unique ID for the current turn.
    pub turn_id: String,

    /// Consecutive autocompact failures. Reset on success.
    /// Used as a circuit breaker to stop retrying when the context is
    /// irrecoverably over the limit.
    pub consecutive_failures: u32,
}

impl Default for AutoCompactTrackingState {
    fn default() -> Self {
        Self {
            compacted: false,
            turn_counter: 0,
            turn_id: String::new(),
            consecutive_failures: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Context window helpers
// ---------------------------------------------------------------------------

/// Returns the effective context window size minus the max output tokens
/// reserved for summary generation.
pub fn get_effective_context_window_size(model: &str) -> u64 {
    let model_info = get_model_info(model);
    let context_window: u64 = 200_000;

    let max_output = model_info
        .map(|m| m.max_output_tokens as u64)
        .unwrap_or(MAX_OUTPUT_TOKENS_FOR_SUMMARY as u64);

    let reserved = max_output.min(MAX_OUTPUT_TOKENS_FOR_SUMMARY as u64);
    context_window.saturating_sub(reserved)
}

/// Returns the token threshold above which auto-compaction should trigger.
pub fn get_auto_compact_threshold(model: &str) -> u64 {
    let effective = get_effective_context_window_size(model);
    effective.saturating_sub(AUTOCOMPACT_BUFFER_TOKENS)
}

/// Check whether auto-compaction is enabled.
///
/// In the TypeScript reference this consults env vars (`DISABLE_COMPACT`,
/// `DISABLE_AUTO_COMPACT`) and user config. For the Rust port we default to
/// enabled and expose a simple toggle.
pub fn is_auto_compact_enabled() -> bool {
    // Check environment overrides.
    if std::env::var("DISABLE_COMPACT")
        .map(|v| is_truthy(&v))
        .unwrap_or(false)
    {
        return false;
    }
    if std::env::var("DISABLE_AUTO_COMPACT")
        .map(|v| is_truthy(&v))
        .unwrap_or(false)
    {
        return false;
    }
    true
}

/// Check whether the conversation should be auto-compacted.
///
/// Returns `true` when auto-compact is enabled and the estimated token count
/// exceeds the auto-compact threshold for the given model.
pub async fn auto_compact_check(messages: &[Message], model: &str) -> bool {
    if !is_auto_compact_enabled() {
        return false;
    }

    let token_count = estimate_message_tokens(messages);
    let threshold = get_auto_compact_threshold(model);

    tracing::debug!(
        token_count,
        threshold,
        "autocompact: checking token usage against threshold"
    );

    token_count >= threshold
}

/// Calculate token warning state for the current conversation.
#[derive(Debug, Clone)]
pub struct TokenWarningState {
    pub percent_left: u32,
    pub is_above_warning_threshold: bool,
    pub is_above_error_threshold: bool,
    pub is_above_auto_compact_threshold: bool,
    pub is_at_blocking_limit: bool,
}

/// Compute the token warning state for display purposes.
pub fn calculate_token_warning_state(token_usage: u64, model: &str) -> TokenWarningState {
    let threshold = if is_auto_compact_enabled() {
        get_auto_compact_threshold(model)
    } else {
        get_effective_context_window_size(model)
    };

    let percent_left = if threshold > 0 {
        ((threshold.saturating_sub(token_usage) as f64 / threshold as f64) * 100.0)
            .max(0.0)
            .round() as u32
    } else {
        0
    };

    let warning_threshold = threshold.saturating_sub(super::WARNING_THRESHOLD_BUFFER_TOKENS);
    let error_threshold = threshold.saturating_sub(super::ERROR_THRESHOLD_BUFFER_TOKENS);

    let is_above_auto_compact_threshold =
        is_auto_compact_enabled() && token_usage >= get_auto_compact_threshold(model);

    let actual_context_window = get_effective_context_window_size(model);
    let blocking_limit = actual_context_window.saturating_sub(MANUAL_COMPACT_BUFFER_TOKENS);

    TokenWarningState {
        percent_left,
        is_above_warning_threshold: token_usage >= warning_threshold,
        is_above_error_threshold: token_usage >= error_threshold,
        is_above_auto_compact_threshold,
        is_at_blocking_limit: token_usage >= blocking_limit,
    }
}

/// Check if the circuit breaker has tripped for auto-compaction.
pub fn is_circuit_breaker_tripped(tracking: &AutoCompactTrackingState) -> bool {
    tracking.consecutive_failures >= MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn is_truthy(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "1" | "true" | "yes")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_window_is_positive() {
        let size = get_effective_context_window_size("gpt-4o");
        assert!(size > 0);
    }

    #[test]
    fn threshold_less_than_window() {
        let model = "gpt-4o";
        let threshold = get_auto_compact_threshold(model);
        let window = get_effective_context_window_size(model);
        assert!(threshold < window);
    }

    #[test]
    fn circuit_breaker_trips_at_max() {
        let tracking = AutoCompactTrackingState {
            consecutive_failures: MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES,
            ..Default::default()
        };
        assert!(is_circuit_breaker_tripped(&tracking));
    }

    #[test]
    fn circuit_breaker_does_not_trip_below() {
        let tracking = AutoCompactTrackingState {
            consecutive_failures: 0,
            ..Default::default()
        };
        assert!(!is_circuit_breaker_tripped(&tracking));
    }
}
