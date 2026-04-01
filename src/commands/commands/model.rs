//! Model / inference commands: /model, /cost, /usage, /fast, /effort, /advisor.
//!
//! Ported from ref/commands/model, cost, usage, fast, effort, advisor.ts.

use crate::types::command::{
    Command, CommandAvailability, LocalCommandData, LocalJsxCommandData,
};

use super::base;

// ============================================================================
// /model
// ============================================================================

/// `/model` -- Set the AI model.
///
/// Type: local-jsx
/// The description in the TS reference is dynamic (`currently <model>`);
/// we use a static description that the TUI can augment at render time.
pub fn model() -> Command {
    let mut b = base("model", "Set the AI model for ThunderCode");
    b.argument_hint = Some("[model]".into());
    // In the TS reference, `immediate` is dynamic based on
    // `shouldInferenceConfigCommandBeImmediate()`. Default to false;
    // the runtime can override.
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /cost
// ============================================================================

/// `/cost` -- Show total cost and duration of the current session.
///
/// Type: local
/// Hidden for the provider subscribers (they don't see per-token costs).
pub fn cost() -> Command {
    let b = base(
        "cost",
        "Show the total cost and duration of the current session",
    );
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /usage
// ============================================================================

/// `/usage` -- Show plan usage limits.
///
/// Type: local-jsx | Availability: primary-ai only
pub fn usage() -> Command {
    let mut b = base("usage", "Show plan usage limits");
    b.availability = Some(vec![CommandAvailability::Authenticated]);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /fast
// ============================================================================

/// `/fast` -- Toggle fast mode (Haiku-only).
///
/// Type: local-jsx | Availability: primary-ai, console
/// Hidden and disabled when fast mode feature is not available.
pub fn fast() -> Command {
    let mut b = base("fast", "Toggle fast mode (Haiku only)");
    b.availability = Some(vec![
        CommandAvailability::Authenticated,
        CommandAvailability::Authenticated,
    ]);
    b.argument_hint = Some("[on|off]".into());
    // In the TS reference, isEnabled/isHidden/immediate are dynamic.
    // Default to enabled; runtime can gate on feature flag.
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /effort
// ============================================================================

/// `/effort` -- Set effort level for model usage.
///
/// Type: local-jsx
pub fn effort() -> Command {
    let mut b = base("effort", "Set effort level for model usage");
    b.argument_hint = Some("[low|medium|high|max|auto]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /advisor
// ============================================================================

/// `/advisor` -- Configure the advisor model.
///
/// Type: local | Argument: [<model>|off]
/// Hidden by default; gated on canUserConfigureAdvisor() at runtime.
pub fn advisor() -> Command {
    let mut b = base("advisor", "Configure the advisor model");
    b.argument_hint = Some("[<model>|off]".into());
    // In the TS reference, gated on canUserConfigureAdvisor().
    // Default to hidden/disabled; runtime can enable.
    b.is_hidden = Some(true);
    b.is_enabled = Some(false);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /extra-usage (interactive)
// ============================================================================

/// `/extra-usage` -- Configure extra usage to keep working when limits are hit.
///
/// Type: local-jsx
/// In the TS reference, gated on isOverageProvisioningAllowed().
pub fn extra_usage() -> Command {
    let b = base(
        "extra-usage",
        "Configure extra usage to keep working when limits are hit",
    );
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /extra-usage (non-interactive)
// ============================================================================

/// `/extra-usage` (non-interactive variant) -- Configure extra usage.
///
/// Type: local | Hidden when in interactive mode.
pub fn extra_usage_non_interactive() -> Command {
    let mut b = base(
        "extra-usage",
        "Configure extra usage to keep working when limits are hit",
    );
    b.is_hidden = Some(true);
    b.is_enabled = Some(false);
    Command::Local(LocalCommandData {
        base: b,
        supports_non_interactive: true,
    })
}

// ============================================================================
// /rate-limit-options
// ============================================================================

/// `/rate-limit-options` -- Show options when rate limit is reached.
///
/// Type: local-jsx | Hidden: true (internal use only)
pub fn rate_limit_options() -> Command {
    let mut b = base("rate-limit-options", "Show options when rate limit is reached");
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /passes
// ============================================================================

/// `/passes` -- Share a free week of ThunderCode with friends.
///
/// Type: local-jsx | Hidden by default (gated on eligibility).
pub fn passes() -> Command {
    let mut b = base(
        "passes",
        "Share a free week of ThunderCode with friends",
    );
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}
