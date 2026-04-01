//! Settings commands: /theme, /output-style, /keybindings, /permissions,
//! /privacy-settings, /config, /remote-env, /sandbox.
//!
//! Ported from ref/commands/theme, output-style, keybindings, permissions,
//! privacy-settings, config, remote-env, sandbox-toggle.

use crate::types::command::{
    Command, CommandAvailability, LocalCommandData, LocalJsxCommandData,
};

use super::{base, base_with_aliases};

// ============================================================================
// /theme
// ============================================================================

/// `/theme` -- Change the theme.
///
/// Type: local-jsx
pub fn theme() -> Command {
    let b = base("theme", "Change the theme");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /output-style
// ============================================================================

/// `/output-style` -- Deprecated; redirects to /config.
///
/// Type: local-jsx | Hidden: true
pub fn output_style() -> Command {
    let mut b = base(
        "output-style",
        "Deprecated: use /config to change output style",
    );
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /keybindings
// ============================================================================

/// `/keybindings` -- Open or create keybindings configuration file.
///
/// Type: local
/// In the TS reference, gated on isKeybindingCustomizationEnabled().
pub fn keybindings() -> Command {
    let b = base(
        "keybindings",
        "Open or create your keybindings configuration file",
    );
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: false,
    })
}

// ============================================================================
// /permissions
// ============================================================================

/// `/permissions` -- Manage allow & deny tool permission rules.
///
/// Type: local-jsx | Aliases: allowed-tools
pub fn permissions() -> Command {
    let b = base_with_aliases(
        "permissions",
        "Manage allow & deny tool permission rules",
        vec!["allowed-tools"],
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /privacy-settings
// ============================================================================

/// `/privacy-settings` -- View and update privacy settings.
///
/// Type: local-jsx
/// In the TS reference, gated on isConsumerSubscriber().
pub fn privacy_settings() -> Command {
    let b = base("privacy-settings", "View and update your privacy settings");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /config
// ============================================================================

/// `/config` -- Open config panel.
///
/// Type: local-jsx | Aliases: settings
pub fn config() -> Command {
    let b = base_with_aliases("config", "Open config panel", vec!["settings"]);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /remote-env
// ============================================================================

/// `/remote-env` -- Configure the default remote environment for teleport sessions.
///
/// Type: local-jsx | Availability: primary-ai
/// Hidden and disabled by default; runtime enables when conditions are met.
pub fn remote_env() -> Command {
    let mut b = base(
        "remote-env",
        "Configure the default remote environment for teleport sessions",
    );
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    // In the TS reference, gated on isAuthenticated() && isPolicyAllowed('allow_remote_sessions').
    b.is_hidden = Some(true);
    b.is_enabled = Some(false);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /sandbox
// ============================================================================

/// `/sandbox` -- Configure sandbox settings.
///
/// Type: local-jsx | Immediate: true
/// Hidden on unsupported platforms.
pub fn sandbox() -> Command {
    let mut b = base("sandbox", "Configure sandbox settings");
    b.argument_hint = Some("exclude \"command pattern\"".into());
    b.immediate = Some(true);
    // In the TS reference, hidden on unsupported platforms.
    // Default to hidden; runtime enables on supported platforms.
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}
