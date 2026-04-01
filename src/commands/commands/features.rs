//! Feature commands: /plan, /compact, /tasks, /vim, /think-back, /thinkback-play.
//!
//! Ported from ref/commands/plan, compact, tasks, vim, thinkback, thinkback-play.

use crate::types::command::{Command, LocalCommandData, LocalJsxCommandData};

use super::{base, base_with_aliases};

// ============================================================================
// /plan
// ============================================================================

/// `/plan` -- Enable plan mode or view the current session plan.
///
/// Type: local-jsx
pub fn plan() -> Command {
    let mut b = base("plan", "Enable plan mode or view the current session plan");
    b.argument_hint = Some("[open|<description>]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /compact
// ============================================================================

/// `/compact` -- Clear conversation history but keep a summary in context.
///
/// Type: local
/// In the TS reference, gated on !DISABLE_COMPACT env.
pub fn compact() -> Command {
    let mut b = base(
        "compact",
        "Clear conversation history but keep a summary in context. Optional: /compact [instructions for summarization]",
    );
    b.argument_hint = Some("<optional custom summarization instructions>".into());
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /tasks
// ============================================================================

/// `/tasks` -- List and manage background tasks.
///
/// Type: local-jsx | Aliases: bashes
pub fn tasks() -> Command {
    let b = base_with_aliases(
        "tasks",
        "List and manage background tasks",
        vec!["bashes"],
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /vim
// ============================================================================

/// `/vim` -- Toggle between Vim and Normal editing modes.
///
/// Type: local
pub fn vim() -> Command {
    let b = base("vim", "Toggle between Vim and Normal editing modes");
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /think-back
// ============================================================================

/// `/think-back` -- Your 2025 ThunderCode Year in Review.
///
/// Type: local-jsx
/// In the TS reference, gated on the tengu_thinkback feature gate.
pub fn think_back() -> Command {
    let b = base("think-back", "Your 2025 ThunderCode Year in Review");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /thinkback-play
// ============================================================================

/// `/thinkback-play` -- Play the thinkback animation.
///
/// Type: local | Hidden: true
/// Internal command called by the thinkback skill.
pub fn thinkback_play() -> Command {
    let mut b = base("thinkback-play", "Play the thinkback animation");
    b.is_hidden = Some(true);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}
