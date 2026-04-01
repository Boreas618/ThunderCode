//! System commands: /help, /clear, /exit, /upgrade, /feedback,
//! /release-notes, /stats, /doctor, /login, /logout, /install-github-app,
//! /install-slack-app, /desktop, /mobile, /chrome, /stickers, /heapdump,
//! /version, /terminal-setup.
//!
//! Ported from ref/commands/help, clear, exit, upgrade, feedback,
//! release-notes, stats, doctor, login, logout, install-github-app,
//! install-slack-app, desktop, mobile, chrome, stickers, heapdump, version,
//! terminalSetup, status.

use crate::types::command::{
    Command, CommandAvailability, LocalCommandData, LocalJsxCommandData,
};

use super::{base, base_with_aliases};

// ============================================================================
// /help
// ============================================================================

/// `/help` -- Show help and available commands.
///
/// Type: local-jsx
pub fn help() -> Command {
    let b = base("help", "Show help and available commands");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /clear
// ============================================================================

/// `/clear` -- Clear conversation history and free up context.
///
/// Type: local | Aliases: reset, new
pub fn clear() -> Command {
    let b = base_with_aliases(
        "clear",
        "Clear conversation history and free up context",
        vec!["reset", "new"],
    );
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /exit
// ============================================================================

/// `/exit` -- Exit the REPL.
///
/// Type: local-jsx | Aliases: quit | Immediate: true
pub fn exit() -> Command {
    let mut b = base_with_aliases("exit", "Exit the REPL", vec!["quit"]);
    b.immediate = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /upgrade
// ============================================================================

/// `/upgrade` -- Upgrade to Max for higher rate limits and more Opus.
///
/// Type: local-jsx | Availability: primary-ai
pub fn upgrade() -> Command {
    let mut b = base(
        "upgrade",
        "Upgrade to Max for higher rate limits and more Opus",
    );
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /feedback
// ============================================================================

/// `/feedback` -- Submit feedback.
///
/// Type: local-jsx | Aliases: bug
pub fn feedback() -> Command {
    let mut b = base_with_aliases(
        "feedback",
        "Submit feedback about ThunderCode",
        vec!["bug"],
    );
    b.argument_hint = Some("[report]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /release-notes
// ============================================================================

/// `/release-notes` -- View release notes.
///
/// Type: local
pub fn release_notes() -> Command {
    let b = base("release-notes", "View release notes");
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /stats
// ============================================================================

/// `/stats` -- Show usage statistics and activity.
///
/// Type: local-jsx
pub fn stats() -> Command {
    let b = base(
        "stats",
        "Show your ThunderCode usage statistics and activity",
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /doctor
// ============================================================================

/// `/doctor` -- Diagnose and verify installation and settings.
///
/// Type: local-jsx
pub fn doctor() -> Command {
    let b = base(
        "doctor",
        "Diagnose and verify your ThunderCode installation and settings",
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /login
// ============================================================================

/// `/login` -- Sign in with your API key.
///
/// Type: local-jsx
pub fn login() -> Command {
    let b = base("login", "Sign in with your API key");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /logout
// ============================================================================

/// `/logout` -- Sign out.
///
/// Type: local-jsx
pub fn logout() -> Command {
    let b = base("logout", "Sign out");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /install-github-app
// ============================================================================

/// `/install-github-app` -- Set up GitHub Actions for a repository.
///
/// Type: local-jsx | Availability: primary-ai, console
pub fn install_github_app() -> Command {
    let mut b = base(
        "install-github-app",
        "Set up GitHub Actions for a repository",
    );
    b.availability = Some(vec![
        CommandAvailability::Authenticated,
        CommandAvailability::Authenticated,
    ]);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /install-slack-app
// ============================================================================

/// `/install-slack-app` -- Install the Slack integration.
///
/// Type: local | Availability: primary-ai
pub fn install_slack_app() -> Command {
    let mut b = base("install-slack-app", "Install the Slack integration");
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /desktop
// ============================================================================

/// `/desktop` -- Continue in desktop app.
///
/// Type: local-jsx | Aliases: app | Availability: primary-ai
/// Hidden and disabled on unsupported platforms.
pub fn desktop() -> Command {
    let mut b = base_with_aliases(
        "desktop",
        "Continue in desktop app",
        vec!["app"],
    );
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    // In the TS reference, gated on platform (darwin, win32 x64).
    // Default to enabled; runtime can gate on platform.
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /mobile
// ============================================================================

/// `/mobile` -- Show QR code to download the mobile app.
///
/// Type: local-jsx | Aliases: ios, android
pub fn mobile() -> Command {
    let b = base_with_aliases(
        "mobile",
        "Show QR code to download the mobile app",
        vec!["ios", "android"],
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /chrome
// ============================================================================

/// `/chrome` -- Chrome extension settings.
///
/// Type: local-jsx | Availability: primary-ai
pub fn chrome() -> Command {
    let mut b = base("chrome", "Chrome extension settings");
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /stickers
// ============================================================================

/// `/stickers` -- Order ThunderCode stickers.
///
/// Type: local
pub fn stickers() -> Command {
    let b = base("stickers", "Order ThunderCode stickers");
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /heapdump
// ============================================================================

/// `/heapdump` -- Dump the JS heap to ~/Desktop.
///
/// Type: local | Hidden: true
pub fn heapdump() -> Command {
    let mut b = base("heapdump", "Dump the JS heap to ~/Desktop");
    b.is_hidden = Some(true);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /version
// ============================================================================

/// `/version` -- Print the version this session is running.
///
/// Type: local | Hidden by default (ant-only in ref).
pub fn version() -> Command {
    let mut b = base(
        "version",
        "Print the version this session is running",
    );
    // In the TS reference, gated on USER_TYPE === 'ant'.
    b.is_enabled = Some(false);
    b.is_hidden = Some(true);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /terminal-setup
// ============================================================================

/// `/terminal-setup` -- Configure terminal settings for optimal experience.
///
/// Type: local-jsx
pub fn terminal_setup() -> Command {
    let b = base(
        "terminal-setup",
        "Configure terminal settings for optimal experience",
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}
