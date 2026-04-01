//! Tool / integration commands: /agents, /skills, /plugin, /reload-plugins,
//! /mcp, /hooks, /files, /memory, /ide.
//!
//! Ported from ref/commands/agents, skills, plugin, reload-plugins, mcp, hooks,
//! files, memory, ide.

use crate::types::command::{Command, LocalCommandData, LocalJsxCommandData};

use super::base;

// ============================================================================
// /agents
// ============================================================================

/// `/agents` -- Manage agent configurations.
///
/// Type: local-jsx
pub fn agents() -> Command {
    let b = base("agents", "Manage agent configurations");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /skills
// ============================================================================

/// `/skills` -- List available skills.
///
/// Type: local-jsx
pub fn skills() -> Command {
    let b = base("skills", "List available skills");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /plugin
// ============================================================================

/// `/plugin` -- Manage plugins.
///
/// Type: local-jsx
pub fn plugin() -> Command {
    let b = base("plugin", "Manage plugins");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /reload-plugins
// ============================================================================

/// `/reload-plugins` -- Activate pending plugin changes in the current session.
///
/// Type: local
pub fn reload_plugins() -> Command {
    let b = base(
        "reload-plugins",
        "Activate pending plugin changes in the current session",
    );
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /mcp
// ============================================================================

/// `/mcp` -- Manage MCP servers.
///
/// Type: local-jsx | Immediate: true
pub fn mcp() -> Command {
    let mut b = base("mcp", "Manage MCP servers");
    b.immediate = Some(true);
    b.argument_hint = Some("[enable|disable [server-name]]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /hooks
// ============================================================================

/// `/hooks` -- View hook configurations for tool events.
///
/// Type: local-jsx | Immediate: true
pub fn hooks() -> Command {
    let mut b = base("hooks", "View hook configurations for tool events");
    b.immediate = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /files
// ============================================================================

/// `/files` -- List all files currently in context.
///
/// Type: local
/// In the TS reference, gated on USER_TYPE === 'ant'. We default to enabled.
pub fn files() -> Command {
    let b = base("files", "List all files currently in context");
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /memory
// ============================================================================

/// `/memory` -- Edit memory files.
///
/// Type: local-jsx
pub fn memory() -> Command {
    let b = base("memory", "Edit memory files");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /ide
// ============================================================================

/// `/ide` -- Manage IDE integrations and show status.
///
/// Type: local-jsx
pub fn ide() -> Command {
    let mut b = base("ide", "Manage IDE integrations and show status");
    b.argument_hint = Some("[open]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}
