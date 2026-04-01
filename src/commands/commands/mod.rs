//! Individual command definitions grouped by category.
//!
//! Each sub-module exports constructor functions that return `Command` values.
//! The registry calls these at startup to populate the command list.

pub mod session;
pub mod git;
pub mod model;
pub mod tools;
pub mod settings;
pub mod features;
pub mod system;
pub mod review;

/// Helper to build a `CommandBase` with common defaults.
///
/// Most optional fields are `None`; callers override what they need.
pub(crate) fn base(name: &str, description: &str) -> crate::types::command::CommandBase {
    crate::types::command::CommandBase {
        name: name.to_string(),
        description: description.to_string(),
        has_user_specified_description: None,
        availability: None,
        is_enabled: None,
        is_hidden: None,
        aliases: None,
        is_mcp: None,
        argument_hint: None,
        when_to_use: None,
        version: None,
        disable_model_invocation: None,
        user_invocable: None,
        loaded_from: None,
        kind: None,
        immediate: None,
        is_sensitive: None,
        user_facing_name: None,
    }
}

/// Helper: same as `base` but with aliases.
pub(crate) fn base_with_aliases(
    name: &str,
    description: &str,
    aliases: Vec<&str>,
) -> crate::types::command::CommandBase {
    let mut b = base(name, description);
    b.aliases = Some(aliases.into_iter().map(String::from).collect());
    b
}
