//! QuerySource enum -- identifies how a query entered the system.

use serde::{Deserialize, Serialize};

/// Identifies the origin of a user query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuerySource {
    /// Normal interactive prompt.
    User,
    /// `!` prefix shell passthrough.
    BangCommand,
    /// Piped stdin / `--print` mode.
    Stdin,
    /// Initial prompt provided via `--prompt` or `-p` flag.
    InitialPrompt,
    /// Slash-command (e.g. `/commit`, `/review`).
    SlashCommand,
    /// Skill tool invocation.
    Skill,
    /// Agent SDK / non-interactive.
    Sdk,
    /// MCP resource subscription event.
    McpSubscription,
    /// Cross-session injected message.
    CrossSession,
    /// Tick / heartbeat (proactive mode).
    Tick,
    /// Resume from a previous session.
    Resume,
}

impl std::fmt::Display for QuerySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::User => "user",
            Self::BangCommand => "bang_command",
            Self::Stdin => "stdin",
            Self::InitialPrompt => "initial_prompt",
            Self::SlashCommand => "slash_command",
            Self::Skill => "skill",
            Self::Sdk => "sdk",
            Self::McpSubscription => "mcp_subscription",
            Self::CrossSession => "cross_session",
            Self::Tick => "tick",
            Self::Resume => "resume",
        };
        f.write_str(s)
    }
}
