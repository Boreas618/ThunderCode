//! Permission pattern matching.
//!
//! Ported from ref/utils/permissions/shellRuleMatching.ts` and
//! `ref/utils/permissions/permissionRuleParser.ts`.
//!
//! Supports three kinds of rule matching:
//! - **Exact**: the rule content must equal the command literally.
//! - **Prefix**: legacy `command:*` syntax -- the command must start with `command`.
//! - **Wildcard**: glob-style `*` matching (e.g., `git *`, `npm run *`).

use regex::Regex;
use std::collections::HashMap;

use crate::types::permissions::PermissionRuleValue;

// ============================================================================
// Legacy tool-name aliases
// ============================================================================

/// Maps legacy tool names to their current canonical names.
fn legacy_tool_name_aliases() -> &'static HashMap<&'static str, &'static str> {
    use std::sync::OnceLock;
    static ALIASES: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("Task", "Agent");
        m.insert("KillShell", "TaskStop");
        m.insert("AgentOutputTool", "TaskOutput");
        m.insert("BashOutputTool", "TaskOutput");
        m
    })
}

/// Normalize a potentially-legacy tool name to its canonical form.
pub fn normalize_legacy_tool_name(name: &str) -> String {
    legacy_tool_name_aliases()
        .get(name)
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.to_string())
}

// ============================================================================
// Rule content escaping
// ============================================================================

/// Escape parentheses and backslashes in rule content for safe storage.
///
/// Escaping order: backslashes first, then parentheses.
pub fn escape_rule_content(content: &str) -> String {
    content
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Reverse the escaping done by [`escape_rule_content`].
pub fn unescape_rule_content(content: &str) -> String {
    content
        .replace("\\(", "(")
        .replace("\\)", ")")
        .replace("\\\\", "\\")
}

// ============================================================================
// Rule string parsing  (TS: permissionRuleParser.ts)
// ============================================================================

/// Find the index of the first unescaped occurrence of `ch`.
/// A character is escaped if preceded by an odd number of backslashes.
fn find_first_unescaped(s: &str, ch: char) -> Option<usize> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == ch as u8 {
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 {
                j -= 1;
                if bytes[j] == b'\\' {
                    backslash_count += 1;
                } else {
                    break;
                }
            }
            if backslash_count % 2 == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Find the index of the *last* unescaped occurrence of `ch`.
fn find_last_unescaped(s: &str, ch: char) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).rev() {
        if bytes[i] == ch as u8 {
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 {
                j -= 1;
                if bytes[j] == b'\\' {
                    backslash_count += 1;
                } else {
                    break;
                }
            }
            if backslash_count % 2 == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Parse a rule string like `"Bash(npm install)"` into a [`PermissionRuleValue`].
///
/// Handles escaped parentheses in content and empty/wildcard content.
pub fn permission_rule_value_from_string(rule_string: &str) -> PermissionRuleValue {
    let open = find_first_unescaped(rule_string, '(');
    let close = find_last_unescaped(rule_string, ')');

    match (open, close) {
        (Some(open_idx), Some(close_idx))
            if close_idx > open_idx && close_idx == rule_string.len() - 1 =>
        {
            let tool_name = &rule_string[..open_idx];
            let raw_content = &rule_string[open_idx + 1..close_idx];

            // Malformed: missing tool name, e.g. "(foo)"
            if tool_name.is_empty() {
                return PermissionRuleValue {
                    tool_name: normalize_legacy_tool_name(rule_string),
                    rule_content: None,
                };
            }

            // Empty content or standalone wildcard => tool-wide rule
            if raw_content.is_empty() || raw_content == "*" {
                return PermissionRuleValue {
                    tool_name: normalize_legacy_tool_name(tool_name),
                    rule_content: None,
                };
            }

            PermissionRuleValue {
                tool_name: normalize_legacy_tool_name(tool_name),
                rule_content: Some(unescape_rule_content(raw_content)),
            }
        }
        _ => PermissionRuleValue {
            tool_name: normalize_legacy_tool_name(rule_string),
            rule_content: None,
        },
    }
}

/// Serialize a [`PermissionRuleValue`] back to its string form.
pub fn permission_rule_value_to_string(rv: &PermissionRuleValue) -> String {
    match &rv.rule_content {
        Some(content) => {
            let escaped = escape_rule_content(content);
            format!("{}({})", rv.tool_name, escaped)
        }
        None => rv.tool_name.clone(),
    }
}

// ============================================================================
// Shell permission rules  (TS: shellRuleMatching.ts)
// ============================================================================

/// Parsed permission rule -- discriminated by match type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPermissionRule {
    /// Exact literal match.
    Exact { command: String },
    /// Legacy `:*` suffix -- prefix match.
    Prefix { prefix: String },
    /// Glob-style pattern with `*` wildcards.
    Wildcard { pattern: String },
}

/// Extract the prefix from legacy `command:*` syntax.
/// Returns `None` if the rule does not use that syntax.
pub fn permission_rule_extract_prefix(rule: &str) -> Option<String> {
    if rule.ends_with(":*") && rule.len() > 2 {
        Some(rule[..rule.len() - 2].to_string())
    } else {
        None
    }
}

/// Returns `true` if `pattern` contains unescaped `*` wildcards
/// (excluding the legacy `:*` suffix).
pub fn has_wildcards(pattern: &str) -> bool {
    if pattern.ends_with(":*") {
        return false;
    }
    let bytes = pattern.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'*' {
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 {
                j -= 1;
                if bytes[j] == b'\\' {
                    backslash_count += 1;
                } else {
                    break;
                }
            }
            if backslash_count % 2 == 0 {
                return true;
            }
        }
    }
    false
}

/// Match a command against a wildcard permission pattern.
///
/// `*` matches any sequence of characters (including none).
/// `\*` matches a literal asterisk.
/// `\\` matches a literal backslash.
///
/// When a pattern ends with ` *` (space + single unescaped wildcard),
/// the trailing space-and-args portion is made optional so that
/// `git *` matches both `git add` and bare `git`.
pub fn match_wildcard_pattern(pattern: &str, command: &str) -> bool {
    match_wildcard_pattern_impl(pattern, command, false)
}

/// Case-insensitive variant of [`match_wildcard_pattern`].
pub fn match_wildcard_pattern_case_insensitive(pattern: &str, command: &str) -> bool {
    match_wildcard_pattern_impl(pattern, command, true)
}

fn match_wildcard_pattern_impl(pattern: &str, command: &str, case_insensitive: bool) -> bool {
    let trimmed = pattern.trim();

    // Phase 1: process escape sequences into sentinel placeholders.
    const STAR_PH: &str = "\x00ESCAPED_STAR\x00";
    const BSLASH_PH: &str = "\x00ESCAPED_BACKSLASH\x00";

    let mut processed = String::with_capacity(trimmed.len());
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                '*' => {
                    processed.push_str(STAR_PH);
                    i += 2;
                    continue;
                }
                '\\' => {
                    processed.push_str(BSLASH_PH);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        processed.push(chars[i]);
        i += 1;
    }

    // Count unescaped stars (before regex conversion).
    let unescaped_star_count = processed.matches('*').count();

    // Phase 2: escape regex-special chars except `*`.
    let mut escaped = String::with_capacity(processed.len() * 2);
    for ch in processed.chars() {
        if ch == '*' {
            escaped.push('*');
        } else if ".+?^${}()|[]\\'\"".contains(ch) {
            escaped.push('\\');
            escaped.push(ch);
        } else {
            escaped.push(ch);
        }
    }

    // Phase 3: convert `*` -> `.*`
    let with_wildcards = escaped.replace('*', ".*");

    // Phase 4: restore placeholders -> escaped regex literals.
    let mut regex_pattern = with_wildcards
        .replace(STAR_PH, "\\*")
        .replace(BSLASH_PH, "\\\\");

    // Phase 5: trailing ` *` (single unescaped wildcard) → optional `( .*)?`.
    if regex_pattern.ends_with(" .*") && unescaped_star_count == 1 {
        let new_len = regex_pattern.len() - 3;
        regex_pattern.truncate(new_len);
        regex_pattern.push_str("( .*)?");
    }

    // Phase 6: compile and test.
    let flags = if case_insensitive { "(?si)" } else { "(?s)" };
    let full = format!("{flags}^{regex_pattern}$");
    Regex::new(&full)
        .map(|re| re.is_match(command))
        .unwrap_or(false)
}

/// Parse a permission rule string into a structured [`ShellPermissionRule`].
pub fn parse_permission_rule(rule: &str) -> ShellPermissionRule {
    // Legacy :* prefix syntax first (backwards compatibility).
    if let Some(prefix) = permission_rule_extract_prefix(rule) {
        return ShellPermissionRule::Prefix { prefix };
    }
    // Wildcard syntax.
    if has_wildcards(rule) {
        return ShellPermissionRule::Wildcard {
            pattern: rule.to_string(),
        };
    }
    // Exact match.
    ShellPermissionRule::Exact {
        command: rule.to_string(),
    }
}

/// Check whether `command` matches a parsed shell permission rule.
pub fn command_matches_rule(rule: &ShellPermissionRule, command: &str) -> bool {
    match rule {
        ShellPermissionRule::Exact { command: c } => c == command,
        ShellPermissionRule::Prefix { prefix } => {
            command == prefix.as_str() || command.starts_with(&format!("{prefix} "))
        }
        ShellPermissionRule::Wildcard { pattern } => match_wildcard_pattern(pattern, command),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- permission_rule_value_from_string ------------------------------------

    #[test]
    fn parse_tool_name_only() {
        let rv = permission_rule_value_from_string("Bash");
        assert_eq!(rv.tool_name, "Bash");
        assert_eq!(rv.rule_content, None);
    }

    #[test]
    fn parse_tool_with_content() {
        let rv = permission_rule_value_from_string("Bash(npm install)");
        assert_eq!(rv.tool_name, "Bash");
        assert_eq!(rv.rule_content.as_deref(), Some("npm install"));
    }

    #[test]
    fn parse_escaped_parens() {
        let rv = permission_rule_value_from_string(r#"Bash(python -c "print\(1\)")"#);
        assert_eq!(rv.tool_name, "Bash");
        assert!(rv.rule_content.is_some());
        assert_eq!(rv.rule_content.as_deref(), Some(r#"python -c "print(1)""#));
    }

    #[test]
    fn parse_empty_content_becomes_tool_only() {
        let rv = permission_rule_value_from_string("Bash()");
        assert_eq!(rv.tool_name, "Bash");
        assert_eq!(rv.rule_content, None);
    }

    #[test]
    fn parse_wildcard_content_becomes_tool_only() {
        let rv = permission_rule_value_from_string("Bash(*)");
        assert_eq!(rv.tool_name, "Bash");
        assert_eq!(rv.rule_content, None);
    }

    #[test]
    fn parse_legacy_tool_name() {
        let rv = permission_rule_value_from_string("Task");
        assert_eq!(rv.tool_name, "Agent");
    }

    // -- roundtrip -------------------------------------------------------------

    #[test]
    fn roundtrip_rule_value() {
        let rv = PermissionRuleValue {
            tool_name: "Bash".into(),
            rule_content: Some("npm install".into()),
        };
        let s = permission_rule_value_to_string(&rv);
        assert_eq!(s, "Bash(npm install)");
        let parsed = permission_rule_value_from_string(&s);
        assert_eq!(parsed.tool_name, rv.tool_name);
        assert_eq!(parsed.rule_content, rv.rule_content);
    }

    // -- wildcard matching ----------------------------------------------------

    #[test]
    fn wildcard_simple() {
        assert!(match_wildcard_pattern("git *", "git add"));
        assert!(match_wildcard_pattern("git *", "git commit -m 'msg'"));
    }

    #[test]
    fn wildcard_bare_command_matches_trailing_star() {
        // `git *` matches bare `git` (trailing space+wildcard is optional).
        assert!(match_wildcard_pattern("git *", "git"));
    }

    #[test]
    fn wildcard_no_match() {
        assert!(!match_wildcard_pattern("git *", "npm install"));
    }

    #[test]
    fn wildcard_escaped_star() {
        // `\*` matches a literal asterisk.
        assert!(match_wildcard_pattern("echo \\*", "echo *"));
        assert!(!match_wildcard_pattern("echo \\*", "echo foo"));
    }

    #[test]
    fn wildcard_multi_star() {
        // Multi-wildcard: trailing space+wildcard is NOT optional.
        assert!(match_wildcard_pattern("* run *", "npm run dev"));
        assert!(!match_wildcard_pattern("* run *", "npm run"));
    }

    // -- shell permission rule ------------------------------------------------

    #[test]
    fn parse_exact_rule() {
        assert_eq!(
            parse_permission_rule("npm install"),
            ShellPermissionRule::Exact {
                command: "npm install".into()
            }
        );
    }

    #[test]
    fn parse_prefix_rule() {
        assert_eq!(
            parse_permission_rule("npm:*"),
            ShellPermissionRule::Prefix {
                prefix: "npm".into()
            }
        );
    }

    #[test]
    fn parse_wildcard_rule() {
        assert_eq!(
            parse_permission_rule("git *"),
            ShellPermissionRule::Wildcard {
                pattern: "git *".into()
            }
        );
    }

    #[test]
    fn command_matches_exact() {
        let rule = parse_permission_rule("npm install");
        assert!(command_matches_rule(&rule, "npm install"));
        assert!(!command_matches_rule(&rule, "npm run"));
    }

    #[test]
    fn command_matches_prefix() {
        let rule = parse_permission_rule("npm:*");
        assert!(command_matches_rule(&rule, "npm install"));
        assert!(command_matches_rule(&rule, "npm run dev"));
        assert!(command_matches_rule(&rule, "npm")); // bare prefix
        assert!(!command_matches_rule(&rule, "npx create-react-app"));
    }

    // -- escape / unescape ----------------------------------------------------

    #[test]
    fn escape_unescape_round_trip() {
        let original = r#"python -c "print(1)""#;
        let escaped = escape_rule_content(original);
        assert!(escaped.contains("\\("));
        let back = unescape_rule_content(&escaped);
        assert_eq!(back, original);
    }
}
