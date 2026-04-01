//! Main permission checking flow.
//!
//! Ported from ref/utils/permissions/permissions.ts`:
//! `hasPermissionsToUseToolInner` (steps 1--3).
//!
//! This orchestrates the full permission pipeline:
//!
//! 1. Check deny rules (tool-level).
//! 2. Check ask rules (tool-level).
//! 3. Delegate to tool-specific `check_permissions` callback.
//! 4. Handle deny / ask / safety-check results from the tool.
//! 5. Check bypass mode.
//! 6. Check allow rules (tool-level).
//! 7. Convert passthrough to ask.
//! 8. Apply mode-based transformations (dontAsk -> deny, auto -> classifier).

use crate::types::permissions::{
    PermissionBehavior, PermissionDecisionReason, PermissionMode, PermissionResult,
    ToolPermissionContext,
};

use crate::permissions::rules::{evaluate_tool_rules, get_ask_rule_for_tool, get_deny_rule_for_tool, tool_always_allowed_rule, RuleEvalResult};

// ============================================================================
// Tool permission callback
// ============================================================================

/// The result that a tool's `check_permissions` implementation returns.
/// This is a simplified version of `PermissionResult` that the tool produces.
#[derive(Debug, Clone)]
pub struct ToolPermissionCheckResult {
    pub behavior: PermissionBehavior,
    pub message: Option<String>,
    pub updated_input: Option<serde_json::Value>,
    pub decision_reason: Option<PermissionDecisionReason>,
    /// Passthrough means the tool has no opinion -- delegate to the framework.
    pub is_passthrough: bool,
}

impl ToolPermissionCheckResult {
    /// The tool explicitly allows.
    pub fn allow(updated_input: Option<serde_json::Value>) -> Self {
        Self {
            behavior: PermissionBehavior::Allow,
            message: None,
            updated_input,
            decision_reason: None,
            is_passthrough: false,
        }
    }

    /// The tool explicitly denies.
    pub fn deny(message: impl Into<String>, reason: PermissionDecisionReason) -> Self {
        Self {
            behavior: PermissionBehavior::Deny,
            message: Some(message.into()),
            updated_input: None,
            decision_reason: Some(reason),
            is_passthrough: false,
        }
    }

    /// The tool wants the user asked.
    pub fn ask(message: impl Into<String>, reason: Option<PermissionDecisionReason>) -> Self {
        Self {
            behavior: PermissionBehavior::Ask,
            message: Some(message.into()),
            updated_input: None,
            decision_reason: reason,
            is_passthrough: false,
        }
    }

    /// The tool has no opinion -- passthrough to framework rules.
    pub fn passthrough(message: impl Into<String>) -> Self {
        Self {
            behavior: PermissionBehavior::Ask,
            message: Some(message.into()),
            updated_input: None,
            decision_reason: None,
            is_passthrough: true,
        }
    }
}

// ============================================================================
// Main checker
// ============================================================================

/// Run the permission pipeline for a given tool invocation.
///
/// # Arguments
///
/// - `tool_name` -- canonical tool name (e.g. `"Bash"`, `"FileEdit"`).
/// - `context` -- the current [`ToolPermissionContext`] from app state.
/// - `tool_result` -- the result from calling the tool's own `check_permissions`.
///
/// # Returns
///
/// A [`PermissionResult`] that is either Allow, Deny, or Ask.
pub fn check_permissions(
    tool_name: &str,
    context: &ToolPermissionContext,
    tool_result: ToolPermissionCheckResult,
) -> PermissionResult {
    // === Step 1a: Entire tool denied by rule ===
    if let Some(deny_rule) = get_deny_rule_for_tool(context, tool_name) {
        return PermissionResult::deny(
            format!("Permission to use {} has been denied.", tool_name),
            PermissionDecisionReason::Rule { rule: deny_rule },
        );
    }

    // === Step 1b: Entire tool has an ask rule ===
    if let Some(ask_rule) = get_ask_rule_for_tool(context, tool_name) {
        return PermissionResult::Ask {
            message: create_permission_request_message(tool_name, None),
            updated_input: None,
            decision_reason: Some(PermissionDecisionReason::Rule { rule: ask_rule }),
            suggestions: None,
            blocked_path: None,
            metadata: None,
            is_bash_security_check_for_misparsing: None,
            pending_classifier_check: None,
            content_blocks: None,
        };
    }

    // === Step 1c/1d: Tool implementation denied ===
    if tool_result.behavior == PermissionBehavior::Deny {
        let msg = tool_result
            .message
            .unwrap_or_else(|| format!("Permission to use {} has been denied.", tool_name));
        let reason = tool_result.decision_reason.unwrap_or(
            PermissionDecisionReason::Other {
                reason: "denied by tool".into(),
            },
        );
        return PermissionResult::deny(msg, reason);
    }

    // === Step 1f: Content-specific ask rules from tool ===
    // When a tool's checkPermissions returns ask + rule with ruleBehavior=ask,
    // this must be respected even in bypass mode.
    if tool_result.behavior == PermissionBehavior::Ask && !tool_result.is_passthrough {
        if let Some(PermissionDecisionReason::Rule { ref rule }) = tool_result.decision_reason {
            if rule.rule_behavior == PermissionBehavior::Ask {
                return make_ask_result(tool_name, &tool_result);
            }
        }
    }

    // === Step 1g: Safety checks are bypass-immune ===
    if tool_result.behavior == PermissionBehavior::Ask && !tool_result.is_passthrough {
        if let Some(PermissionDecisionReason::SafetyCheck { .. }) = &tool_result.decision_reason {
            return make_ask_result(tool_name, &tool_result);
        }
    }

    // === Step 2a: Bypass mode ===
    let should_bypass = context.mode == PermissionMode::BypassPermissions
        || (context.mode == PermissionMode::Plan && context.is_bypass_permissions_mode_available);
    if should_bypass {
        return PermissionResult::Allow {
            updated_input: tool_result.updated_input,
            user_modified: None,
            decision_reason: Some(PermissionDecisionReason::Mode {
                mode: context.mode,
            }),
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        };
    }

    // === Step 2b: Entire tool always-allowed ===
    if let Some(allow_rule) = tool_always_allowed_rule(context, tool_name) {
        return PermissionResult::Allow {
            updated_input: tool_result.updated_input,
            user_modified: None,
            decision_reason: Some(PermissionDecisionReason::Rule { rule: allow_rule }),
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        };
    }

    // === Step 3: Handle tool result ===
    // If the tool explicitly allowed (e.g. via its own content-level rules), propagate.
    if tool_result.behavior == PermissionBehavior::Allow {
        return PermissionResult::Allow {
            updated_input: tool_result.updated_input,
            user_modified: None,
            decision_reason: tool_result.decision_reason,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        };
    }

    // Convert passthrough or ask to a final ask result.
    let msg = tool_result
        .message
        .unwrap_or_else(|| create_permission_request_message(tool_name, None));

    let decision = if tool_result.is_passthrough {
        tool_result.decision_reason.clone()
    } else {
        tool_result.decision_reason.clone()
    };

    // === Mode transformations ===
    // dontAsk mode: convert ask -> deny
    if context.mode == PermissionMode::DontAsk {
        return PermissionResult::deny(
            format!(
                "Permission to use {} was denied because the current mode (Don't Ask) does not allow interactive prompts.",
                tool_name
            ),
            PermissionDecisionReason::Mode {
                mode: PermissionMode::DontAsk,
            },
        );
    }

    PermissionResult::Ask {
        message: msg,
        updated_input: tool_result.updated_input,
        decision_reason: decision,
        suggestions: None,
        blocked_path: None,
        metadata: None,
        is_bash_security_check_for_misparsing: None,
        pending_classifier_check: None,
        content_blocks: None,
    }
}

/// Build a human-readable permission request message.
pub fn create_permission_request_message(
    tool_name: &str,
    decision_reason: Option<&PermissionDecisionReason>,
) -> String {
    if let Some(reason) = decision_reason {
        match reason {
            PermissionDecisionReason::Rule { rule } => {
                let rule_str = crate::permissions::matcher::permission_rule_value_to_string(&rule.rule_value);
                format!(
                    "Permission rule '{}' requires approval for this {} command",
                    rule_str, tool_name
                )
            }
            PermissionDecisionReason::Mode { mode } => {
                format!(
                    "Current permission mode ({:?}) requires approval for this {} command",
                    mode, tool_name
                )
            }
            PermissionDecisionReason::SafetyCheck { reason, .. } => reason.clone(),
            PermissionDecisionReason::WorkingDir { reason } => reason.clone(),
            PermissionDecisionReason::Other { reason } => reason.clone(),
            PermissionDecisionReason::Hook {
                hook_name, reason, ..
            } => {
                if let Some(r) = reason {
                    format!("Hook '{}' blocked this action: {}", hook_name, r)
                } else {
                    format!(
                        "Hook '{}' requires approval for this {} command",
                        hook_name, tool_name
                    )
                }
            }
            PermissionDecisionReason::AsyncAgent { reason } => reason.clone(),
            _ => format!(
                "ThunderCode requested permissions to use {}, but you haven't granted it yet.",
                tool_name
            ),
        }
    } else {
        format!(
            "ThunderCode requested permissions to use {}, but you haven't granted it yet.",
            tool_name
        )
    }
}

/// Convenience: evaluate only the rule-based pipeline steps (no mode
/// transformations, no classifier). Used by callers that need to know
/// whether rules alone would block an action.
pub fn check_rule_based_permissions(
    tool_name: &str,
    context: &ToolPermissionContext,
) -> Option<PermissionResult> {
    match evaluate_tool_rules(context, tool_name) {
        RuleEvalResult::Deny(rule) => Some(PermissionResult::deny(
            format!("Permission to use {} has been denied.", tool_name),
            PermissionDecisionReason::Rule { rule },
        )),
        RuleEvalResult::Ask(rule) => Some(PermissionResult::Ask {
            message: create_permission_request_message(tool_name, None),
            updated_input: None,
            decision_reason: Some(PermissionDecisionReason::Rule { rule }),
            suggestions: None,
            blocked_path: None,
            metadata: None,
            is_bash_security_check_for_misparsing: None,
            pending_classifier_check: None,
            content_blocks: None,
        }),
        RuleEvalResult::Allow(_) | RuleEvalResult::NoMatch => None,
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn make_ask_result(tool_name: &str, tool_result: &ToolPermissionCheckResult) -> PermissionResult {
    let msg = tool_result
        .message
        .clone()
        .unwrap_or_else(|| create_permission_request_message(tool_name, None));
    PermissionResult::Ask {
        message: msg,
        updated_input: tool_result.updated_input.clone(),
        decision_reason: tool_result.decision_reason.clone(),
        suggestions: None,
        blocked_path: None,
        metadata: None,
        is_bash_security_check_for_misparsing: None,
        pending_classifier_check: None,
        content_blocks: None,
    }
}

/// Get the prose description for a permission behavior.
pub fn rule_behavior_description(behavior: PermissionBehavior) -> &'static str {
    match behavior {
        PermissionBehavior::Allow => "allowed",
        PermissionBehavior::Deny => "denied",
        PermissionBehavior::Ask => "asked for confirmation for",
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::permissions::ToolPermissionContext;

    fn ctx_with_deny(tool: &str) -> ToolPermissionContext {
        let mut ctx = ToolPermissionContext::default();
        ctx.always_deny_rules
            .insert("session".into(), vec![tool.into()]);
        ctx
    }

    fn ctx_with_allow(tool: &str) -> ToolPermissionContext {
        let mut ctx = ToolPermissionContext::default();
        ctx.always_allow_rules
            .insert("session".into(), vec![tool.into()]);
        ctx
    }

    fn ctx_with_ask(tool: &str) -> ToolPermissionContext {
        let mut ctx = ToolPermissionContext::default();
        ctx.always_ask_rules
            .insert("session".into(), vec![tool.into()]);
        ctx
    }

    #[test]
    fn deny_rule_produces_deny() {
        let ctx = ctx_with_deny("Bash");
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Deny);
    }

    #[test]
    fn allow_rule_produces_allow() {
        let ctx = ctx_with_allow("Bash");
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Allow);
    }

    #[test]
    fn ask_rule_produces_ask() {
        let ctx = ctx_with_ask("Bash");
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Ask);
    }

    #[test]
    fn bypass_mode_overrides_passthrough() {
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::BypassPermissions;
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Allow);
    }

    #[test]
    fn bypass_mode_does_not_override_deny_rule() {
        let mut ctx = ctx_with_deny("Bash");
        ctx.mode = PermissionMode::BypassPermissions;
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Deny);
    }

    #[test]
    fn tool_deny_is_respected() {
        let ctx = ToolPermissionContext::default();
        let tool_result = ToolPermissionCheckResult::deny(
            "unsafe",
            PermissionDecisionReason::Other {
                reason: "test".into(),
            },
        );
        let result = check_permissions("Bash", &ctx, tool_result);
        assert_eq!(result.behavior(), PermissionBehavior::Deny);
    }

    #[test]
    fn dont_ask_mode_converts_ask_to_deny() {
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::DontAsk;
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Deny);
    }

    #[test]
    fn passthrough_becomes_ask() {
        let ctx = ToolPermissionContext::default();
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Ask);
    }

    #[test]
    fn safety_check_ask_is_bypass_immune() {
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::BypassPermissions;
        let tool_result = ToolPermissionCheckResult {
            behavior: PermissionBehavior::Ask,
            message: Some("sensitive file".into()),
            updated_input: None,
            decision_reason: Some(PermissionDecisionReason::SafetyCheck {
                reason: "sensitive file".into(),
                classifier_approvable: false,
            }),
            is_passthrough: false,
        };
        let result = check_permissions("FileEdit", &ctx, tool_result);
        assert_eq!(result.behavior(), PermissionBehavior::Ask);
    }

    #[test]
    fn deny_beats_allow_rule() {
        let mut ctx = ToolPermissionContext::default();
        ctx.always_deny_rules
            .insert("userSettings".into(), vec!["Bash".into()]);
        ctx.always_allow_rules
            .insert("session".into(), vec!["Bash".into()]);
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Deny);
    }

    #[test]
    fn plan_mode_with_bypass_available() {
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::Plan;
        ctx.is_bypass_permissions_mode_available = true;
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Allow);
    }

    #[test]
    fn plan_mode_without_bypass_falls_through() {
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::Plan;
        ctx.is_bypass_permissions_mode_available = false;
        let result = check_permissions("Bash", &ctx, ToolPermissionCheckResult::passthrough(""));
        assert_eq!(result.behavior(), PermissionBehavior::Ask);
    }

    #[test]
    fn tool_explicit_allow_propagates() {
        let ctx = ToolPermissionContext::default();
        let tool_result = ToolPermissionCheckResult::allow(None);
        let result = check_permissions("Bash", &ctx, tool_result);
        assert_eq!(result.behavior(), PermissionBehavior::Allow);
    }

    #[test]
    fn content_specific_ask_rule_is_bypass_immune() {
        use crate::types::permissions::{PermissionRule, PermissionRuleSource, PermissionRuleValue};
        let mut ctx = ToolPermissionContext::default();
        ctx.mode = PermissionMode::BypassPermissions;
        let tool_result = ToolPermissionCheckResult {
            behavior: PermissionBehavior::Ask,
            message: Some("npm publish requires approval".into()),
            updated_input: None,
            decision_reason: Some(PermissionDecisionReason::Rule {
                rule: PermissionRule {
                    source: PermissionRuleSource::UserSettings,
                    rule_behavior: PermissionBehavior::Ask,
                    rule_value: PermissionRuleValue {
                        tool_name: "Bash".into(),
                        rule_content: Some("npm publish".into()),
                    },
                },
            }),
            is_passthrough: false,
        };
        let result = check_permissions("Bash", &ctx, tool_result);
        assert_eq!(result.behavior(), PermissionBehavior::Ask);
    }
}
