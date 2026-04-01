//! Branded ID types for sessions, agents, tasks, and teams.
//!
//! Ported from ref/types/ids.ts

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// SessionId
// ---------------------------------------------------------------------------

/// A session ID uniquely identifies a ThunderCode session.
/// Wraps a UUID v4 string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// Generate a new random session ID (UUID v4).
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Wrap an existing string as a `SessionId`.
    /// Use sparingly -- prefer `SessionId::new()` when possible.
    pub fn from_str(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Access the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

impl std::ops::Deref for SessionId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

/// An agent ID uniquely identifies a subagent within a session.
///
/// Format: `a` + optional `<label>-` + 16 hex characters.
/// Pattern: `^a(?:.+-)?[0-9a-f]{16}$`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

/// Validation pattern for agent IDs.
const AGENT_ID_PATTERN: &str = r"^a(?:.+-)?[0-9a-f]{16}$";

impl AgentId {
    /// Create a new random agent ID (no label).
    ///
    /// Format: `a` + 16 random hex characters.
    pub fn new() -> Self {
        Self(format!("a{}", random_hex_16()))
    }

    /// Create a new agent ID with a label prefix.
    ///
    /// Format: `a<label>-` + 16 random hex characters.
    pub fn with_label(label: &str) -> Self {
        Self(format!("a{}-{}", label, random_hex_16()))
    }

    /// Validate and wrap a raw string as an `AgentId`.
    /// Returns `None` if the string does not match the expected pattern.
    pub fn try_from_str(s: &str) -> Option<Self> {
        let re = regex::Regex::new(AGENT_ID_PATTERN).expect("valid regex");
        if re.is_match(s) {
            Some(Self(s.to_owned()))
        } else {
            None
        }
    }

    /// Cast a raw string to `AgentId` without validation.
    /// Use sparingly -- prefer `try_from_str` or `new`.
    pub fn from_str_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Access the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> Self {
        id.0
    }
}

impl std::ops::Deref for AgentId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// TaskId
// ---------------------------------------------------------------------------

/// A task ID uniquely identifies a background task.
///
/// Format: `<prefix>` + 8 alphanumeric characters where the prefix
/// is determined by `TaskType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Wrap an existing string as a `TaskId`.
    pub fn from_str(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<TaskId> for String {
    fn from(id: TaskId) -> Self {
        id.0
    }
}

impl std::ops::Deref for TaskId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// TeamId
// ---------------------------------------------------------------------------

/// A team ID uniquely identifies a swarm/team of agents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TeamId(String);

impl TeamId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_str(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TeamId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TeamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for TeamId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<TeamId> for String {
    fn from(id: TeamId) -> Self {
        id.0
    }
}

impl std::ops::Deref for TeamId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate 16 random hex characters using the `nanoid` crate with hex alphabet.
fn random_hex_16() -> String {
    let alphabet: Vec<char> = "0123456789abcdef".chars().collect();
    nanoid::nanoid!(16, &alphabet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_roundtrip() {
        let id = SessionId::new();
        let s: String = id.clone().into();
        assert_eq!(id.as_str(), &s);
    }

    #[test]
    fn agent_id_pattern_matches() {
        let id = AgentId::new();
        assert!(AgentId::try_from_str(id.as_str()).is_some());
    }

    #[test]
    fn agent_id_with_label() {
        let id = AgentId::with_label("bash");
        assert!(AgentId::try_from_str(id.as_str()).is_some());
        assert!(id.as_str().starts_with("abash-"));
    }

    #[test]
    fn agent_id_rejects_invalid() {
        assert!(AgentId::try_from_str("not-an-agent-id").is_none());
        assert!(AgentId::try_from_str("a12345").is_none()); // too short
    }
}
