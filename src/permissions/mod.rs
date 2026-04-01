//! ThunderCode permission system.
//!
//! This crate implements the permission logic for tool invocations:
//! rule evaluation, pattern matching, bash command classification,
//! sandbox restrictions, denial tracking, and the top-level permission
//! checker that orchestrates them all.
//!
//! Ported from the TypeScript reference in `ref/utils/permissions/`.

pub mod checker;
pub mod classifier;
pub mod denial_tracking;
pub mod matcher;
pub mod rules;
pub mod sandbox;

// Re-export the most commonly used items.
pub use checker::check_permissions;
pub use classifier::{classify_bash_command, CommandSafety};
pub use denial_tracking::DenialTracker;
pub use matcher::{match_wildcard_pattern, parse_permission_rule, ShellPermissionRule};
pub use rules::{
    get_allow_rules, get_ask_rules, get_deny_rules, get_rule_by_contents_for_tool_name,
    tool_always_allowed_rule, RuleEvalResult,
};
pub use sandbox::{SandboxConfig, SandboxFsConfig};
