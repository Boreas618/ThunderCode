//! Permission rule evaluation.
//!
//! Ported from ref/utils/permissions/permissions.ts`:
//! - `getAllowRules`, `getDenyRules`, `getAskRules`
//! - `toolMatchesRule`, `toolAlwaysAllowedRule`, `getDenyRuleForTool`, `getAskRuleForTool`
//! - `getRuleByContentsForToolName`
//!
//! ## Rule precedence
//!
//! 1. **Deny** rules beat everything.
//! 2. **Ask** rules override allow.
//! 3. **Allow** rules permit the action.
//!
//! ## Source precedence
//!
//! Policy > User > Project > Local > Flag > Session > Command > CliArg.
//! This is encoded by the order in [`PERMISSION_RULE_SOURCES`].

use std::collections::HashMap;

use crate::types::permissions::{
    PermissionBehavior, PermissionRule, PermissionRuleSource, ToolPermissionContext,
};

use crate::permissions::matcher::permission_rule_value_from_string;

// ============================================================================
// Source ordering
// ============================================================================

/// All rule sources, ordered from highest precedence to lowest.
pub const PERMISSION_RULE_SOURCES: &[PermissionRuleSource] = &[
    PermissionRuleSource::PolicySettings,
    PermissionRuleSource::UserSettings,
    PermissionRuleSource::ProjectSettings,
    PermissionRuleSource::LocalSettings,
    PermissionRuleSource::FlagSettings,
    PermissionRuleSource::Session,
    PermissionRuleSource::Command,
    PermissionRuleSource::CliArg,
];

// ============================================================================
// Rule extraction
// ============================================================================

/// Flatten the `always_allow_rules` map into a `Vec<PermissionRule>`.
pub fn get_allow_rules(context: &ToolPermissionContext) -> Vec<PermissionRule> {
    rules_from_map(&context.always_allow_rules, PermissionBehavior::Allow)
}

/// Flatten the `always_deny_rules` map into a `Vec<PermissionRule>`.
pub fn get_deny_rules(context: &ToolPermissionContext) -> Vec<PermissionRule> {
    rules_from_map(&context.always_deny_rules, PermissionBehavior::Deny)
}

/// Flatten the `always_ask_rules` map into a `Vec<PermissionRule>`.
pub fn get_ask_rules(context: &ToolPermissionContext) -> Vec<PermissionRule> {
    rules_from_map(&context.always_ask_rules, PermissionBehavior::Ask)
}

/// Helper: turn `{ "sourceKey": ["ruleString", ...] }` into `Vec<PermissionRule>`.
fn rules_from_map(
    map: &HashMap<String, Vec<String>>,
    behavior: PermissionBehavior,
) -> Vec<PermissionRule> {
    let mut out = Vec::new();
    for source in PERMISSION_RULE_SOURCES {
        let key = source_to_key(*source);
        if let Some(rules) = map.get(key) {
            for rule_string in rules {
                out.push(PermissionRule {
                    source: *source,
                    rule_behavior: behavior,
                    rule_value: permission_rule_value_from_string(rule_string),
                });
            }
        }
    }
    out
}

/// Canonical string key for a [`PermissionRuleSource`] in the rules map.
fn source_to_key(source: PermissionRuleSource) -> &'static str {
    match source {
        PermissionRuleSource::UserSettings => "userSettings",
        PermissionRuleSource::ProjectSettings => "projectSettings",
        PermissionRuleSource::LocalSettings => "localSettings",
        PermissionRuleSource::FlagSettings => "flagSettings",
        PermissionRuleSource::PolicySettings => "policySettings",
        PermissionRuleSource::CliArg => "cliArg",
        PermissionRuleSource::Command => "command",
        PermissionRuleSource::Session => "session",
    }
}

// ============================================================================
// Tool-level matching
// ============================================================================

/// Returns `true` when a tool (identified by `tool_name`) matches a rule that
/// has **no** `rule_content` -- i.e., the rule applies to the entire tool.
///
/// Also handles MCP server-level matching: a rule for `mcp__server1` matches
/// tool `mcp__server1__tool1`.
fn tool_matches_rule(tool_name: &str, rule: &PermissionRule) -> bool {
    // Rule must not have content to match the entire tool.
    if rule.rule_value.rule_content.is_some() {
        return false;
    }

    // Direct name match.
    if rule.rule_value.tool_name == tool_name {
        return true;
    }

    // MCP server-level: rule "mcp__server1" matches tool "mcp__server1__toolX".
    if let Some(rule_server) = mcp_server_name(&rule.rule_value.tool_name) {
        if let Some(tool_server) = mcp_server_name(tool_name) {
            if rule_server == tool_server {
                // Rule is server-level only (no tool component) or wildcard.
                let rule_tool = mcp_tool_name(&rule.rule_value.tool_name);
                if rule_tool.is_none() || rule_tool == Some("*") {
                    return true;
                }
            }
        }
    }

    false
}

/// Extract MCP server name from `"mcp__<server>__<tool>"` or `"mcp__<server>"`.
fn mcp_server_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("mcp__")?;
    Some(rest.split("__").next().unwrap_or(rest))
}

/// Extract MCP tool name from `"mcp__<server>__<tool>"`.
fn mcp_tool_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("mcp__")?;
    let mut parts = rest.splitn(2, "__");
    parts.next(); // skip server
    parts.next()
}

/// Find the first allow rule that matches the entire tool (no content).
pub fn tool_always_allowed_rule(
    context: &ToolPermissionContext,
    tool_name: &str,
) -> Option<PermissionRule> {
    get_allow_rules(context)
        .into_iter()
        .find(|rule| tool_matches_rule(tool_name, rule))
}

/// Find the first deny rule that matches the entire tool.
pub fn get_deny_rule_for_tool(
    context: &ToolPermissionContext,
    tool_name: &str,
) -> Option<PermissionRule> {
    get_deny_rules(context)
        .into_iter()
        .find(|rule| tool_matches_rule(tool_name, rule))
}

/// Find the first ask rule that matches the entire tool.
pub fn get_ask_rule_for_tool(
    context: &ToolPermissionContext,
    tool_name: &str,
) -> Option<PermissionRule> {
    get_ask_rules(context)
        .into_iter()
        .find(|rule| tool_matches_rule(tool_name, rule))
}

/// Find the deny rule for a specific agent type, e.g. `Agent(Explore)`.
pub fn get_deny_rule_for_agent(
    context: &ToolPermissionContext,
    agent_tool_name: &str,
    agent_type: &str,
) -> Option<PermissionRule> {
    get_deny_rules(context).into_iter().find(|rule| {
        rule.rule_value.tool_name == agent_tool_name
            && rule.rule_value.rule_content.as_deref() == Some(agent_type)
    })
}

// ============================================================================
// Content-level matching (rule_content based)
// ============================================================================

/// Build a map from rule content strings to the associated rule for a given
/// `tool_name` and `behavior`. This is the "content-specific" rule index
/// used by tool implementations (Bash, FileEdit, etc.) to check per-command
/// or per-path rules.
pub fn get_rule_by_contents_for_tool_name(
    context: &ToolPermissionContext,
    tool_name: &str,
    behavior: PermissionBehavior,
) -> HashMap<String, PermissionRule> {
    let rules = match behavior {
        PermissionBehavior::Allow => get_allow_rules(context),
        PermissionBehavior::Deny => get_deny_rules(context),
        PermissionBehavior::Ask => get_ask_rules(context),
    };
    let mut map = HashMap::new();
    for rule in rules {
        if rule.rule_value.tool_name == tool_name
            && rule.rule_behavior == behavior
        {
            if let Some(ref content) = rule.rule_value.rule_content {
                map.insert(content.clone(), rule);
            }
        }
    }
    map
}

// ============================================================================
// High-level evaluation result
// ============================================================================

/// The outcome of evaluating all rules for a given tool + input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEvalResult {
    /// A rule explicitly allows the action.
    Allow(PermissionRule),
    /// A rule explicitly denies the action.
    Deny(PermissionRule),
    /// A rule requires interactive confirmation.
    Ask(PermissionRule),
    /// No rule matched -- fall through to next check.
    NoMatch,
}

/// Evaluate the three rule sets (deny > ask > allow) for an entire tool
/// (not content-specific). Returns the first matching result following the
/// most-restrictive-wins precedence.
pub fn evaluate_tool_rules(context: &ToolPermissionContext, tool_name: &str) -> RuleEvalResult {
    // 1. Deny rules first (highest precedence).
    if let Some(rule) = get_deny_rule_for_tool(context, tool_name) {
        return RuleEvalResult::Deny(rule);
    }
    // 2. Ask rules next.
    if let Some(rule) = get_ask_rule_for_tool(context, tool_name) {
        return RuleEvalResult::Ask(rule);
    }
    // 3. Allow rules.
    if let Some(rule) = tool_always_allowed_rule(context, tool_name) {
        return RuleEvalResult::Allow(rule);
    }
    RuleEvalResult::NoMatch
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(
        allow: Vec<(&str, Vec<&str>)>,
        deny: Vec<(&str, Vec<&str>)>,
        ask: Vec<(&str, Vec<&str>)>,
    ) -> ToolPermissionContext {
        let mut ctx = ToolPermissionContext::default();
        for (src, rules) in allow {
            ctx.always_allow_rules
                .insert(src.into(), rules.into_iter().map(String::from).collect());
        }
        for (src, rules) in deny {
            ctx.always_deny_rules
                .insert(src.into(), rules.into_iter().map(String::from).collect());
        }
        for (src, rules) in ask {
            ctx.always_ask_rules
                .insert(src.into(), rules.into_iter().map(String::from).collect());
        }
        ctx
    }

    #[test]
    fn deny_beats_allow() {
        let ctx = make_context(
            vec![("session", vec!["Bash"])],
            vec![("userSettings", vec!["Bash"])],
            vec![],
        );
        match evaluate_tool_rules(&ctx, "Bash") {
            RuleEvalResult::Deny(_) => {} // expected
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn ask_beats_allow() {
        let ctx = make_context(
            vec![("session", vec!["Bash"])],
            vec![],
            vec![("userSettings", vec!["Bash"])],
        );
        match evaluate_tool_rules(&ctx, "Bash") {
            RuleEvalResult::Ask(_) => {} // expected
            other => panic!("expected Ask, got {:?}", other),
        }
    }

    #[test]
    fn allow_when_no_deny_or_ask() {
        let ctx = make_context(vec![("session", vec!["Bash"])], vec![], vec![]);
        match evaluate_tool_rules(&ctx, "Bash") {
            RuleEvalResult::Allow(_) => {} // expected
            other => panic!("expected Allow, got {:?}", other),
        }
    }

    #[test]
    fn no_match_when_empty() {
        let ctx = ToolPermissionContext::default();
        assert_eq!(evaluate_tool_rules(&ctx, "Bash"), RuleEvalResult::NoMatch);
    }

    #[test]
    fn content_rules_not_tool_match() {
        // A rule like "Bash(npm install)" should NOT match tool "Bash" at tool level.
        let ctx = make_context(vec![("session", vec!["Bash(npm install)"])], vec![], vec![]);
        assert_eq!(evaluate_tool_rules(&ctx, "Bash"), RuleEvalResult::NoMatch);
    }

    #[test]
    fn mcp_server_level_rule() {
        let ctx = make_context(
            vec![("session", vec!["mcp__myserver"])],
            vec![],
            vec![],
        );
        match evaluate_tool_rules(&ctx, "mcp__myserver__sometool") {
            RuleEvalResult::Allow(_) => {}
            other => panic!("expected Allow, got {:?}", other),
        }
    }

    #[test]
    fn get_rule_by_contents_returns_content_rules() {
        let ctx = make_context(
            vec![("session", vec!["Bash(npm install)", "Bash(git:*)"])],
            vec![],
            vec![],
        );
        let map = get_rule_by_contents_for_tool_name(&ctx, "Bash", PermissionBehavior::Allow);
        assert!(map.contains_key("npm install"));
        assert!(map.contains_key("git:*"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn deny_rule_for_agent() {
        let ctx = make_context(
            vec![],
            vec![("userSettings", vec!["Agent(Explore)"])],
            vec![],
        );
        let rule = get_deny_rule_for_agent(&ctx, "Agent", "Explore");
        assert!(rule.is_some());
        assert!(get_deny_rule_for_agent(&ctx, "Agent", "Code").is_none());
    }
}
