//! Session commands: /resume, /session, /context, /rewind, /rename, /export,
//! /copy, /add-dir, /btw, /share.
//!
//! Ported from ref/commands/resume, session, context, rewind, rename, export,
//! copy, add-dir, btw, share.

use crate::types::command::{Command, LocalCommandData, LocalJsxCommandData};

use super::{base, base_with_aliases};

/// `/resume` -- Resume a previous conversation.
///
/// Type: local-jsx | Aliases: continue
pub fn resume() -> Command {
    let mut b = base_with_aliases(
        "resume",
        "Resume a previous conversation",
        vec!["continue"],
    );
    b.argument_hint = Some("[conversation id or search term]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/session` -- Show remote session URL and QR code.
///
/// Type: local-jsx | Aliases: remote
/// Hidden and disabled when not in remote mode.
pub fn session() -> Command {
    let mut b = base_with_aliases(
        "session",
        "Show remote session URL and QR code",
        vec!["remote"],
    );
    // In the TS reference this is dynamically gated on getIsRemoteMode().
    // We default to disabled/hidden; the runtime can flip these.
    b.is_enabled = Some(false);
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/context` -- Visualize current context usage as a colored grid (interactive).
///
/// Type: local-jsx
pub fn context() -> Command {
    let b = base("context", "Visualize current context usage as a colored grid");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/context` (non-interactive variant) -- Show current context usage.
///
/// Type: local | Hidden when in interactive mode.
pub fn context_non_interactive() -> Command {
    let mut b = base("context", "Show current context usage");
    b.is_hidden = Some(true);
    b.is_enabled = Some(false);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

/// `/rewind` -- Restore code and/or conversation to a previous point.
///
/// Type: local | Aliases: checkpoint
pub fn rewind() -> Command {
    let mut b = base_with_aliases(
        "rewind",
        "Restore the code and/or conversation to a previous point",
        vec!["checkpoint"],
    );
    b.argument_hint = Some("".into());
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

/// `/rename` -- Rename the current conversation.
///
/// Type: local-jsx | Immediate: true
pub fn rename() -> Command {
    let mut b = base("rename", "Rename the current conversation");
    b.immediate = Some(true);
    b.argument_hint = Some("[name]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/export` -- Export the current conversation to a file or clipboard.
///
/// Type: local-jsx
pub fn export() -> Command {
    let mut b = base(
        "export",
        "Export the current conversation to a file or clipboard",
    );
    b.argument_hint = Some("[filename]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/copy` -- Copy last response to clipboard.
///
/// Type: local-jsx
pub fn copy() -> Command {
    let b = base(
        "copy",
        "Copy last response to clipboard (or /copy N for the Nth-latest)",
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/add-dir` -- Add a new working directory.
///
/// Type: local-jsx
pub fn add_dir() -> Command {
    let mut b = base("add-dir", "Add a new working directory");
    b.argument_hint = Some("<path>".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/btw` -- Ask a quick side question without interrupting the main conversation.
///
/// Type: local-jsx | Immediate: true
pub fn btw() -> Command {
    let mut b = base(
        "btw",
        "Ask a quick side question without interrupting the main conversation",
    );
    b.immediate = Some(true);
    b.argument_hint = Some("<question>".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/color` -- Set the prompt bar color for this session.
///
/// Type: local-jsx | Immediate: true
pub fn color() -> Command {
    let mut b = base("color", "Set the prompt bar color for this session");
    b.immediate = Some(true);
    b.argument_hint = Some("<color|default>".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/tag` -- Toggle a searchable tag on the current session.
///
/// Type: local-jsx | Hidden by default (ant-only in ref).
pub fn tag() -> Command {
    let mut b = base("tag", "Toggle a searchable tag on the current session");
    b.argument_hint = Some("<tag-name>".into());
    // In the TS reference, gated on USER_TYPE === 'ant'.
    b.is_enabled = Some(false);
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

/// `/summary` -- Summarize the current conversation.
///
/// Type: local | Hidden by default (internal command).
pub fn summary() -> Command {
    let mut b = base("summary", "Summarize the current conversation");
    b.is_hidden = Some(true);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}
