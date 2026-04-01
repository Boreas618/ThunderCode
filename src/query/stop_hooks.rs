//! Stop hooks -- post-turn processing before the loop terminates.
//!
//! Ported from ref/query/stopHooks.ts`. Stop hooks run after the model
//! produces an `end_turn` response (no tool calls) and before the query
//! loop returns. They can:
//!
//! - Inject blocking-error messages that force the model to retry.
//! - Prevent continuation entirely (e.g. teammate-idle signal).
//! - Fire-and-forget side effects (memory extraction, prompt suggestions).

use crate::types::messages::{AssistantMessage, Message};

use crate::query::config::QueryConfig;

// ---------------------------------------------------------------------------
// StopHookResult
// ---------------------------------------------------------------------------

/// The result of running stop hooks.
#[derive(Debug, Clone, Default)]
pub struct StopHookResult {
    /// Blocking-error messages injected by hooks. When non-empty, the query
    /// loop appends these to the conversation and continues with the model.
    pub blocking_errors: Vec<Message>,

    /// When `true`, the loop must stop immediately -- no retry, no further
    /// model calls. This is set when a hook explicitly prevents continuation
    /// (e.g. a teammate-idle hook).
    pub prevent_continuation: bool,
}

// ---------------------------------------------------------------------------
// run_stop_hooks
// ---------------------------------------------------------------------------

/// Execute stop hooks against the completed turn.
///
/// `messages_for_query` is the full conversation as sent to the API (post-
/// compaction, post-microcompact). `assistant_messages` are the model
/// responses from this iteration.
///
/// In this initial port, stop hooks are a no-op. The full implementation
/// will:
/// 1. Run user-configured `Stop` hooks (shell commands).
/// 2. Run `TeammateIdle` and `TaskCompleted` hooks when in teammate mode.
/// 3. Fire-and-forget background tasks (memory extraction, auto-dream).
///
/// Returns `StopHookResult` indicating whether to continue, stop, or retry
/// with blocking errors.
pub async fn run_stop_hooks(
    _messages_for_query: &[Message],
    _assistant_messages: &[AssistantMessage],
    _config: &QueryConfig,
    _stop_hook_active: bool,
) -> StopHookResult {
    // TODO: Wire up hook execution once the hooks service is available.
    // The TS implementation:
    // 1. Builds a stop-hook context from messages + system prompt.
    // 2. Calls executeStopHooks() which runs user-configured shell commands.
    // 3. Collects blocking errors and prevent-continuation signals.
    // 4. Runs TeammateIdle/TaskCompleted hooks if in teammate mode.
    // 5. Fires background tasks (prompt suggestion, memory extraction).
    StopHookResult::default()
}
