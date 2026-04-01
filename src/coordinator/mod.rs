//! ThunderCode multi-agent coordinator mode.
//!
//! Provides mode detection, system prompt generation, and tool set management
//! for the coordinator orchestration mode. In coordinator mode, the LLM acts
//! as a supervisor that spawns worker agents to execute tasks in parallel.
//!
//! Ported from: `ref/coordinator/coordinatorMode.ts`

mod prompt;
mod tools;

pub use prompt::{get_coordinator_system_prompt, get_coordinator_user_context};
pub use tools::{
    ASYNC_AGENT_ALLOWED_TOOLS, COORDINATOR_MODE_ALLOWED_TOOLS,
    IN_PROCESS_TEAMMATE_ALLOWED_TOOLS,
};

use std::env;

/// Environment variable that enables coordinator mode.
const COORDINATOR_MODE_ENV: &str = "THUNDERCODE_COORDINATOR_MODE";

/// Check whether the current process is running in coordinator mode.
///
/// Coordinator mode is enabled when the `THUNDERCODE_COORDINATOR_MODE` environment
/// variable is set to a truthy value (`"1"`, `"true"`, `"yes"`).
pub fn is_coordinator_mode() -> bool {
    env::var(COORDINATOR_MODE_ENV)
        .map(|v| is_truthy(&v))
        .unwrap_or(false)
}

/// Match the session's stored mode and flip the environment variable if needed.
///
/// Returns a warning message if the mode was switched, or `None` if no change
/// was necessary. This allows resuming sessions that were started in a different
/// mode than the current process.
pub fn match_session_mode(session_mode: Option<SessionMode>) -> Option<String> {
    let session_mode = session_mode?;
    let current_is_coordinator = is_coordinator_mode();
    let session_is_coordinator = session_mode == SessionMode::Coordinator;

    if current_is_coordinator == session_is_coordinator {
        return None;
    }

    if session_is_coordinator {
        env::set_var(COORDINATOR_MODE_ENV, "1");
    } else {
        env::remove_var(COORDINATOR_MODE_ENV);
    }

    tracing::info!(
        to = ?session_mode,
        "coordinator mode switched to match resumed session"
    );

    if session_is_coordinator {
        Some("Entered coordinator mode to match resumed session.".to_string())
    } else {
        Some("Exited coordinator mode to match resumed session.".to_string())
    }
}

/// The mode a session was started in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    Coordinator,
    Normal,
}

/// Check whether a string is a truthy value.
fn is_truthy(s: &str) -> bool {
    matches!(s, "1" | "true" | "yes" | "TRUE" | "YES" | "True" | "Yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_truthy() {
        assert!(is_truthy("1"));
        assert!(is_truthy("true"));
        assert!(is_truthy("TRUE"));
        assert!(is_truthy("yes"));
        assert!(!is_truthy("0"));
        assert!(!is_truthy("false"));
        assert!(!is_truthy(""));
    }

    #[test]
    fn test_session_mode_serde() {
        let json = serde_json::to_string(&SessionMode::Coordinator).unwrap();
        assert_eq!(json, "\"coordinator\"");

        let mode: SessionMode = serde_json::from_str("\"normal\"").unwrap();
        assert_eq!(mode, SessionMode::Normal);
    }

    #[test]
    fn test_match_session_mode_none() {
        assert!(match_session_mode(None).is_none());
    }
}
