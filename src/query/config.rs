//! Query configuration types.
//!
//! Ported from ref/query/config.ts` and `ref/query/deps.ts`.
//! Immutable configuration snapshotted once at query entry, plus dependency
//! injection for testability.

use crate::types::ids::SessionId;

// ---------------------------------------------------------------------------
// QueryConfig
// ---------------------------------------------------------------------------

/// Immutable configuration snapshotted once at `query()` entry.
///
/// Separating this from the per-iteration `QueryState` and the mutable
/// `ToolUseContext` makes future step-function extraction tractable: a pure
/// reducer can take `(state, event, config)` where config is plain data.
///
/// Feature gates that participate in dead-code elimination are intentionally
/// **not** stored here -- they must stay inline at the guarded blocks.
#[derive(Debug, Clone)]
pub struct QueryConfig {
    /// Session identifier for analytics and transcript correlation.
    pub session_id: SessionId,

    /// The model to use for inference.
    pub model: String,

    /// Maximum output tokens per API call.
    pub max_tokens: u32,

    /// Thinking mode configuration (provider-specific, stored as JSON value).
    pub thinking_config: Option<serde_json::Value>,

    /// Runtime feature gates (env vars, remote config, etc.).
    pub gates: QueryGates,
}

/// Runtime feature gates.
///
/// These are *not* compile-time feature gates -- they correspond to runtime
/// checks (env vars, statsig, etc.) that are snapshotted once at query entry.
#[derive(Debug, Clone)]
pub struct QueryGates {
    /// Whether tools execute while the stream is still active.
    pub streaming_tool_execution: bool,

    /// Whether tool use summaries are emitted (for mobile UI).
    pub emit_tool_use_summaries: bool,

    /// Whether fast mode is enabled.
    pub fast_mode_enabled: bool,

    /// Maximum number of turns before the loop stops.
    pub max_turns: Option<u32>,
}

impl Default for QueryGates {
    fn default() -> Self {
        Self {
            streaming_tool_execution: false,
            emit_tool_use_summaries: false,
            fast_mode_enabled: false,
            max_turns: None,
        }
    }
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            session_id: SessionId::default(),
            model: "gpt-4o".to_owned(),
            max_tokens: 16384,
            thinking_config: None,
            gates: QueryGates::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// QueryDeps
// ---------------------------------------------------------------------------

/// I/O dependency handles injected into `query()`.
///
/// Production code uses `QueryDeps::production()`; tests inject fakes
/// directly. This mirrors the pattern in `ref/query/deps.ts`.
///
/// Scope is intentionally narrow to prove the pattern; additional deps
/// (stop hooks, analytics, queue ops) can be added incrementally.
#[derive(Clone)]
pub struct QueryDeps {
    /// Generate a new UUID string.
    pub uuid_fn: fn() -> String,
}

impl QueryDeps {
    /// Production dependency set.
    pub fn production() -> Self {
        Self {
            uuid_fn: || uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Generate a new UUID.
    pub fn uuid(&self) -> String {
        (self.uuid_fn)()
    }
}

impl Default for QueryDeps {
    fn default() -> Self {
        Self::production()
    }
}

impl std::fmt::Debug for QueryDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryDeps").finish()
    }
}
