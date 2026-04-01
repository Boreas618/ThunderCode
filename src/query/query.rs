//! The core inference loop -- the async generator/stream.
//!
//! Ported from ref/query.ts`. This is the heart of the system: it drives
//! the model, dispatches tool calls, handles compaction, manages error
//! recovery, and enforces token budgets.
//!
//! The loop is a state machine:
//! ```text
//!   ┌───────────────────────────────────────────────────────┐
//!   │                    query_loop                         │
//!   │                                                       │
//!   │  ┌──────┐   stream    ┌───────────┐   tool_use       │
//!   │  │ init ├────────────►│  model    ├───────────┐      │
//!   │  └──────┘             │  stream   │           │      │
//!   │                       └─────┬─────┘           ▼      │
//!   │                             │           ┌──────────┐  │
//!   │                        end_turn         │ execute  │  │
//!   │                             │           │ tools    │  │
//!   │                             ▼           └────┬─────┘  │
//!   │                      ┌─────────────┐         │        │
//!   │                      │ stop hooks  │◄────────┘        │
//!   │                      └──────┬──────┘  (continue)      │
//!   │                             │                          │
//!   │                      ┌──────┴──────┐                   │
//!   │                      │ blocking?   │──yes──► continue  │
//!   │                      └──────┬──────┘                   │
//!   │                             │ no                       │
//!   │                       token budget?                    │
//!   │                        ┌────┴────┐                     │
//!   │                     continue   stop ──► return Done    │
//!   └───────────────────────────────────────────────────────┘
//! ```

use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;

use crate::types::content::ContentBlock;
use crate::types::messages::{AssistantMessage, Message};
use crate::types::tool::ToolProgressData;

use crate::query::compaction::AutoCompactState;
use crate::query::config::{QueryConfig, QueryDeps};
use crate::query::stop_hooks::run_stop_hooks;
use crate::query::token_budget::{BudgetDecision, BudgetTracker};

// ---------------------------------------------------------------------------
// QueryEvent -- the stream item type
// ---------------------------------------------------------------------------

/// Events yielded by the query stream.
///
/// Consumers (TUI, SDK, headless) subscribe to this stream and react to
/// each event type. The ordering is:
///
/// 1. Zero or more `AssistantChunk` / `ToolUse` / `ToolResult` / `Progress`
/// 2. Optionally `CompactStart` / `CompactEnd`
/// 3. Optionally `Error`
/// 4. Exactly one `Done` at the end of the stream.
#[derive(Debug, Clone)]
pub enum QueryEvent {
    /// A (partial or complete) assistant message.
    AssistantMessage(AssistantMessage),

    /// The model wants to call a tool.
    ToolUse {
        tool_use_id: String,
        tool_name: String,
        input: serde_json::Value,
    },

    /// Result of a tool execution, ready to feed back to the model.
    ToolResult {
        tool_use_id: String,
        result: serde_json::Value,
        is_error: bool,
    },

    /// Progress update from a running tool.
    Progress(ToolProgressData),

    /// Auto-compaction started.
    CompactStart,

    /// Auto-compaction completed.
    CompactEnd,

    /// A recoverable API error (rate limit, overloaded, etc.).
    /// Stored as a string because `crate::api::ApiError` contains
    /// `reqwest::Error` which is not `Clone`.
    Error(String),

    /// The turn is complete. No more events will follow.
    Done {
        stop_reason: StopReason,
        turn_count: u32,
    },
}

// ---------------------------------------------------------------------------
// StopReason
// ---------------------------------------------------------------------------

/// Why the query loop terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The model ended the turn naturally (no tool calls).
    Completed,

    /// Maximum turns reached.
    MaxTurns,

    /// The user aborted during streaming.
    AbortedStreaming,

    /// The user aborted during tool execution.
    AbortedTools,

    /// A stop hook prevented continuation.
    StopHookPrevented,

    /// The model hit the context-too-long limit and recovery failed.
    PromptTooLong,

    /// An image or media size error.
    ImageError,

    /// An unrecoverable model/API error.
    ModelError,

    /// The prompt was at the hard blocking limit.
    BlockingLimit,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::Completed => write!(f, "completed"),
            StopReason::MaxTurns => write!(f, "max_turns"),
            StopReason::AbortedStreaming => write!(f, "aborted_streaming"),
            StopReason::AbortedTools => write!(f, "aborted_tools"),
            StopReason::StopHookPrevented => write!(f, "stop_hook_prevented"),
            StopReason::PromptTooLong => write!(f, "prompt_too_long"),
            StopReason::ImageError => write!(f, "image_error"),
            StopReason::ModelError => write!(f, "model_error"),
            StopReason::BlockingLimit => write!(f, "blocking_limit"),
        }
    }
}

// ---------------------------------------------------------------------------
// ContinueReason
// ---------------------------------------------------------------------------

/// Why the previous loop iteration continued instead of terminating.
///
/// Stored on `QueryState::transition` so tests can assert which recovery
/// path fired without inspecting message contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinueReason {
    /// Normal tool-use follow-up: the model called tools, we executed them,
    /// and are now sending the results back for the next model turn.
    NextTurn,

    /// Stop hook injected blocking errors -- retry with those errors.
    StopHookBlocking,

    /// Token budget continuation -- the model ended the turn but hasn't
    /// used its full output budget.
    TokenBudgetContinuation,

    /// max_output_tokens hit -- injected a recovery nudge.
    MaxOutputTokensRecovery { attempt: u32 },

    /// max_output_tokens escalation -- retrying at a higher limit.
    MaxOutputTokensEscalate,

    /// Reactive compaction recovery after prompt-too-long.
    ReactiveCompactRetry,

    /// Context-collapse drain recovery after prompt-too-long.
    CollapseDrainRetry { committed: u32 },
}

// ---------------------------------------------------------------------------
// QueryState
// ---------------------------------------------------------------------------

/// Mutable state carried between loop iterations.
///
/// The loop body destructures this at the top of each iteration so reads
/// stay as bare names. Continue sites write a new `QueryState` value
/// instead of N separate assignments.
#[derive(Debug, Clone)]
pub struct QueryState {
    /// The conversation messages (grows with each iteration).
    pub messages: Vec<Message>,

    /// Auto-compaction tracking state.
    pub auto_compact_tracking: Option<AutoCompactState>,

    /// How many times we have retried after max_output_tokens.
    pub max_output_tokens_recovery_count: u32,

    /// Whether reactive compaction has been attempted this turn.
    pub has_attempted_reactive_compact: bool,

    /// Per-iteration max_output_tokens override (used for escalation).
    pub max_output_tokens_override: Option<u32>,

    /// The current turn number (1-indexed).
    pub turn_count: u32,

    /// Whether stop hooks are active (set after the first blocking-error
    /// retry so hooks know they are in a retry cycle).
    pub stop_hook_active: bool,

    /// Why the previous iteration continued. `None` on the first iteration.
    pub transition: Option<ContinueReason>,
}

impl QueryState {
    /// Create the initial state for a new query.
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            auto_compact_tracking: None,
            max_output_tokens_recovery_count: 0,
            has_attempted_reactive_compact: false,
            max_output_tokens_override: None,
            turn_count: 1,
            stop_hook_active: false,
            transition: None,
        }
    }
}

// ---------------------------------------------------------------------------
// query -- the main entry point
// ---------------------------------------------------------------------------

/// Maximum number of max_output_tokens recovery attempts before giving up.
const MAX_OUTPUT_TOKENS_RECOVERY_LIMIT: u32 = 3;

/// Run the core inference loop.
///
/// Returns a `Stream` of `QueryEvent`s. The caller must `.await` or poll
/// this stream to drive the loop forward.
///
/// # State machine
///
/// Each iteration of the inner loop:
/// 1. Runs auto-compaction if needed.
/// 2. Calls the model via the API.
/// 3. Collects assistant messages and tool-use blocks.
/// 4. If no tool calls (`end_turn`):
///    a. Runs stop hooks; if blocking errors, continues with those.
///    b. Checks token budget; if under budget, continues with nudge.
///    c. Otherwise, emits `Done`.
/// 5. If tool calls: executes tools, collects results, continues.
/// 6. Checks max_turns; if exceeded, emits `Done`.
pub fn query(
    state: QueryState,
    config: QueryConfig,
    deps: QueryDeps,
) -> Pin<Box<dyn Stream<Item = QueryEvent> + Send>> {
    let (tx, rx) = mpsc::channel(64);

    tokio::spawn(async move {
        query_loop(state, config, deps, tx).await;
    });

    Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// The inner loop, running on a spawned task and sending events through `tx`.
async fn query_loop(
    mut state: QueryState,
    config: QueryConfig,
    _deps: QueryDeps,
    tx: mpsc::Sender<QueryEvent>,
) {
    let mut budget_tracker = BudgetTracker::new();

    loop {
        let QueryState {
            ref mut messages,
            ref mut auto_compact_tracking,
            max_output_tokens_recovery_count,
            has_attempted_reactive_compact: _,
            max_output_tokens_override: _,
            turn_count,
            stop_hook_active,
            ref transition,
        } = state;

        tracing::debug!(
            turn = turn_count,
            transition = ?transition,
            message_count = messages.len(),
            "query loop iteration"
        );

        // ---------------------------------------------------------------
        // 1. Auto-compaction
        // ---------------------------------------------------------------
        if let Some(tracking) = auto_compact_tracking.as_mut() {
            match crate::query::compaction::auto_compact(messages, tracking, &config).await {
                Ok(Some(_result)) => {
                    let _ = tx.send(QueryEvent::CompactStart).await;
                    let _ = tx.send(QueryEvent::CompactEnd).await;
                    // In the full implementation, messages would be replaced
                    // with the post-compact messages here.
                }
                Ok(None) => {
                    // No compaction needed.
                }
                Err(e) => {
                    tracing::warn!("auto-compact failed: {e}");
                    if let Some(t) = auto_compact_tracking.as_mut() {
                        t.consecutive_failures += 1;
                    }
                }
            }
        }

        // ---------------------------------------------------------------
        // 2. Call the model
        // ---------------------------------------------------------------
        // In the full implementation, this calls deps.call_model() and
        // streams events. For now, we simulate the end-of-turn path.
        //
        // The model produces assistant messages. If any contain tool_use
        // blocks, we set `needs_follow_up = true`.
        let assistant_messages: Vec<AssistantMessage> = Vec::new();
        let tool_use_blocks: Vec<ToolUseBlock> = Vec::new();
        let needs_follow_up = !tool_use_blocks.is_empty();

        // Emit assistant messages.
        for msg in &assistant_messages {
            let _ = tx.send(QueryEvent::AssistantMessage(msg.clone())).await;
        }

        // ---------------------------------------------------------------
        // 3. Handle end_turn (no tool calls)
        // ---------------------------------------------------------------
        if !needs_follow_up {
            let last_message = assistant_messages.last();

            // 3a. Max output tokens recovery
            let is_max_output_tokens = last_message
                .as_ref()
                .and_then(|m| m.api_error.as_ref())
                .map(|e| e.error_type == "max_output_tokens")
                .unwrap_or(false);

            if is_max_output_tokens
                && max_output_tokens_recovery_count < MAX_OUTPUT_TOKENS_RECOVERY_LIMIT
            {
                tracing::debug!(
                    attempt = max_output_tokens_recovery_count + 1,
                    "max_output_tokens recovery"
                );
                // Append assistant messages + recovery nudge, then continue.
                let mut next_messages = state.messages.clone();
                next_messages.extend(assistant_messages.into_iter().map(Message::Assistant));
                // The recovery nudge would be appended here as a meta user message.

                state = QueryState {
                    messages: next_messages,
                    auto_compact_tracking: state.auto_compact_tracking,
                    max_output_tokens_recovery_count: max_output_tokens_recovery_count + 1,
                    has_attempted_reactive_compact: state.has_attempted_reactive_compact,
                    max_output_tokens_override: None,
                    turn_count,
                    stop_hook_active: false,
                    transition: Some(ContinueReason::MaxOutputTokensRecovery {
                        attempt: max_output_tokens_recovery_count + 1,
                    }),
                };
                continue;
            }

            // 3b. Skip stop hooks for API errors.
            let is_api_error = last_message
                .as_ref()
                .and_then(|m| m.api_error.as_ref())
                .is_some();

            if is_api_error {
                let _ = tx
                    .send(QueryEvent::Done {
                        stop_reason: StopReason::Completed,
                        turn_count,
                    })
                    .await;
                return;
            }

            // 3c. Run stop hooks.
            let hook_result =
                run_stop_hooks(&state.messages, &assistant_messages, &config, stop_hook_active)
                    .await;

            if hook_result.prevent_continuation {
                let _ = tx
                    .send(QueryEvent::Done {
                        stop_reason: StopReason::StopHookPrevented,
                        turn_count,
                    })
                    .await;
                return;
            }

            if !hook_result.blocking_errors.is_empty() {
                // Append assistant messages + blocking errors, then continue.
                let mut next_messages = state.messages.clone();
                next_messages.extend(assistant_messages.into_iter().map(Message::Assistant));
                next_messages.extend(hook_result.blocking_errors);

                state = QueryState {
                    messages: next_messages,
                    auto_compact_tracking: state.auto_compact_tracking,
                    max_output_tokens_recovery_count: 0,
                    has_attempted_reactive_compact: state.has_attempted_reactive_compact,
                    max_output_tokens_override: None,
                    turn_count,
                    stop_hook_active: true,
                    transition: Some(ContinueReason::StopHookBlocking),
                };
                continue;
            }

            // 3d. Token budget check.
            let budget_decision = budget_tracker.check(
                None, // TODO: wire up actual budget from bootstrap state
                0,    // TODO: wire up actual turn output tokens
            );

            match budget_decision {
                BudgetDecision::Continue { nudge_message, .. } => {
                    tracing::debug!("token budget continuation: {nudge_message}");
                    let mut next_messages = state.messages.clone();
                    next_messages.extend(assistant_messages.into_iter().map(Message::Assistant));
                    // Nudge message would be appended as a meta user message.

                    state = QueryState {
                        messages: next_messages,
                        auto_compact_tracking: state.auto_compact_tracking,
                        max_output_tokens_recovery_count: 0,
                        has_attempted_reactive_compact: false,
                        max_output_tokens_override: None,
                        turn_count,
                        stop_hook_active: false,
                        transition: Some(ContinueReason::TokenBudgetContinuation),
                    };
                    continue;
                }
                BudgetDecision::Stop { event } => {
                    if let Some(evt) = event {
                        tracing::debug!(
                            continuations = evt.continuation_count,
                            pct = evt.pct,
                            diminishing = evt.diminishing_returns,
                            "token budget completed"
                        );
                    }
                }
            }

            // 3e. Normal completion.
            let _ = tx
                .send(QueryEvent::Done {
                    stop_reason: StopReason::Completed,
                    turn_count,
                })
                .await;
            return;
        }

        // ---------------------------------------------------------------
        // 4. Execute tools
        // ---------------------------------------------------------------
        for tool_block in &tool_use_blocks {
            let _ = tx
                .send(QueryEvent::ToolUse {
                    tool_use_id: tool_block.id.clone(),
                    tool_name: tool_block.name.clone(),
                    input: tool_block.input.clone(),
                })
                .await;

            // TODO: Actually execute the tool via the tool runner.
            // For now, emit a placeholder error result.
            let _ = tx
                .send(QueryEvent::ToolResult {
                    tool_use_id: tool_block.id.clone(),
                    result: serde_json::json!({"error": "tool execution not yet wired"}),
                    is_error: true,
                })
                .await;
        }

        // ---------------------------------------------------------------
        // 5. Check max turns
        // ---------------------------------------------------------------
        let next_turn_count = turn_count + 1;
        if let Some(max_turns) = config.gates.max_turns {
            if next_turn_count > max_turns {
                let _ = tx
                    .send(QueryEvent::Done {
                        stop_reason: StopReason::MaxTurns,
                        turn_count: next_turn_count,
                    })
                    .await;
                return;
            }
        }

        // ---------------------------------------------------------------
        // 6. Build next state and continue
        // ---------------------------------------------------------------
        // In the full implementation:
        // - Append assistant messages to the conversation.
        // - Append tool results as user messages.
        // - Increment turn counter.
        // - Handle attachments and memory prefetch.
        let mut next_messages = state.messages.clone();
        next_messages.extend(assistant_messages.into_iter().map(Message::Assistant));
        // Tool result messages would be appended here.

        if let Some(tracking) = state.auto_compact_tracking.as_mut() {
            if tracking.compacted {
                tracking.turn_counter += 1;
            }
        }

        state = QueryState {
            messages: next_messages,
            auto_compact_tracking: state.auto_compact_tracking,
            max_output_tokens_recovery_count: 0,
            has_attempted_reactive_compact: false,
            max_output_tokens_override: None,
            turn_count: next_turn_count,
            stop_hook_active: false,
            transition: Some(ContinueReason::NextTurn),
        };
    }
}

// ---------------------------------------------------------------------------
// ToolUseBlock (internal convenience)
// ---------------------------------------------------------------------------

/// A tool_use block extracted from an assistant message.
///
/// This is a lightweight struct used within the query loop; the full
/// `ContentBlock::ToolUse` variant carries more context.
#[derive(Debug, Clone)]
struct ToolUseBlock {
    id: String,
    name: String,
    input: serde_json::Value,
}

/// Extract tool_use blocks from assistant message content.
fn _extract_tool_use_blocks(messages: &[AssistantMessage]) -> Vec<ToolUseBlock> {
    let mut blocks = Vec::new();
    for msg in messages {
        for block in &msg.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                blocks.push(ToolUseBlock {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
        }
    }
    blocks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn empty_conversation_completes_immediately() {
        let state = QueryState::new(vec![]);
        let config = QueryConfig::default();
        let deps = QueryDeps::production();

        let mut stream = query(state, config, deps);
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        // Should get exactly one Done event.
        assert!(!events.is_empty());
        match events.last().unwrap() {
            QueryEvent::Done { stop_reason, .. } => {
                assert_eq!(*stop_reason, StopReason::Completed);
            }
            other => panic!("expected Done, got: {other:?}"),
        }
    }

    #[test]
    fn stop_reason_display() {
        assert_eq!(StopReason::Completed.to_string(), "completed");
        assert_eq!(StopReason::MaxTurns.to_string(), "max_turns");
        assert_eq!(
            StopReason::AbortedStreaming.to_string(),
            "aborted_streaming"
        );
        assert_eq!(StopReason::ModelError.to_string(), "model_error");
    }

    #[test]
    fn query_state_new() {
        let state = QueryState::new(vec![]);
        assert_eq!(state.turn_count, 1);
        assert!(state.transition.is_none());
        assert!(!state.stop_hook_active);
        assert_eq!(state.max_output_tokens_recovery_count, 0);
    }

    #[test]
    fn continue_reason_variants() {
        // Smoke test that all variants are constructible.
        let reasons = vec![
            ContinueReason::NextTurn,
            ContinueReason::StopHookBlocking,
            ContinueReason::TokenBudgetContinuation,
            ContinueReason::MaxOutputTokensRecovery { attempt: 1 },
            ContinueReason::MaxOutputTokensEscalate,
            ContinueReason::ReactiveCompactRetry,
            ContinueReason::CollapseDrainRetry { committed: 5 },
        ];
        assert_eq!(reasons.len(), 7);
    }
}
