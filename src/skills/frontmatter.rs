//! Skill frontmatter parsing.
//!
//! Parses YAML frontmatter from markdown skill files. Ported from
//! ref/utils/frontmatterParser.ts and ref/skills/loadSkillsDir.ts.

use anyhow::{Context, Result};
use regex::Regex;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// SkillFrontmatter
// ---------------------------------------------------------------------------

/// Parsed frontmatter from a skill markdown file.
#[derive(Debug, Clone, Default)]
pub struct SkillFrontmatter {
    /// Display name override for the skill.
    pub name: Option<String>,
    /// Short description of what the skill does.
    pub description: Option<String>,
    /// Detailed usage scenarios / when the model should invoke this skill.
    pub when_to_use: Option<String>,
    /// Named arguments the skill accepts (from `arguments:` field).
    pub arg_names: Option<Vec<String>>,
    /// Tool names allowed when this skill is active.
    pub allowed_tools: Option<Vec<String>>,
    /// Model alias or name (e.g. "haiku", "sonnet", "opus").
    pub model: Option<String>,
    /// Execution context: "inline" (default) or "fork" (run as sub-agent).
    pub context: Option<String>,
    /// Agent type to use when forked.
    pub agent: Option<String>,
    /// Effort level for agents.
    pub effort: Option<String>,
    /// Glob patterns for file paths this skill applies to.
    pub paths: Option<Vec<String>>,
    /// Hooks to register when this skill is invoked.
    pub hooks: Option<serde_json::Value>,
    /// Skill version string.
    pub version: Option<String>,
    /// Whether users can invoke this skill by typing /skill-name.
    pub user_invocable: Option<bool>,
    /// Whether to disable this skill from being invoked by models.
    pub disable_model_invocation: Option<bool>,
    /// Hint text for command arguments.
    pub argument_hint: Option<String>,
}

// ---------------------------------------------------------------------------
// YAML frontmatter regex
// ---------------------------------------------------------------------------

/// Regex matching YAML frontmatter delimited by `---`.
static FRONTMATTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^---\s*\n([\s\S]*?)---\s*\n?").unwrap());

/// Characters that require quoting in YAML values (glob patterns, etc.).
static YAML_SPECIAL_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[{}\[\]*&#!|>%@`]|: "#).unwrap());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a skill markdown file, extracting frontmatter and body content.
///
/// Returns `(frontmatter, body)` where body is the markdown content after the
/// frontmatter block. If no frontmatter is present, returns default frontmatter
/// and the full content.
pub fn parse_skill_file(content: &str) -> Result<(SkillFrontmatter, String)> {
    let (raw, body) = split_frontmatter(content);
    let fm = parse_raw_frontmatter(&raw)?;
    Ok((fm, body))
}

/// Extract a short description from the first paragraph of markdown content.
///
/// Used as a fallback when frontmatter doesn't include a description.
pub fn extract_description_from_markdown(content: &str, fallback_label: &str) -> String {
    // Take the first non-empty, non-heading line.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Truncate to a reasonable length.
        let desc = if trimmed.len() > 200 {
            format!("{}...", &trimmed[..197])
        } else {
            trimmed.to_string()
        };
        return desc;
    }
    format!("{fallback_label}: {}", content.chars().take(60).collect::<String>())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Split raw markdown into (frontmatter_text, body).
fn split_frontmatter(markdown: &str) -> (String, String) {
    if let Some(caps) = FRONTMATTER_REGEX.captures(markdown) {
        let full_match = caps.get(0).unwrap();
        let fm_text = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let body = markdown[full_match.end()..].to_string();
        (fm_text, body)
    } else {
        (String::new(), markdown.to_string())
    }
}

/// Pre-process frontmatter text to quote values containing special YAML chars.
///
/// This allows glob patterns like `**/*.{ts,tsx}` to survive YAML parsing.
fn quote_problematic_values(frontmatter_text: &str) -> String {
    let mut result = Vec::new();
    // Regex for simple `key: value` lines (no indentation, not list items).
    let kv_re = Regex::new(r"^([a-zA-Z_-]+):\s+(.+)$").unwrap();

    for line in frontmatter_text.lines() {
        if let Some(caps) = kv_re.captures(line) {
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();

            // Skip already-quoted values.
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                result.push(line.to_string());
                continue;
            }

            // Quote if it contains special YAML characters.
            if YAML_SPECIAL_CHARS.is_match(value) {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                result.push(format!("{key}: \"{escaped}\""));
                continue;
            }
        }
        result.push(line.to_string());
    }

    result.join("\n")
}

/// Parse raw YAML frontmatter text into a `SkillFrontmatter`.
fn parse_raw_frontmatter(text: &str) -> Result<SkillFrontmatter> {
    if text.trim().is_empty() {
        return Ok(SkillFrontmatter::default());
    }

    // Try parsing as-is first; retry with quoting on failure.
    let value: serde_yaml::Value = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            let quoted = quote_problematic_values(text);
            serde_yaml::from_str(&quoted)
                .context("failed to parse YAML frontmatter even after quoting")?
        }
    };

    let mapping = match value {
        serde_yaml::Value::Mapping(m) => m,
        _ => return Ok(SkillFrontmatter::default()),
    };

    Ok(SkillFrontmatter {
        name: get_string(&mapping, "name"),
        description: get_string(&mapping, "description"),
        when_to_use: get_string(&mapping, "when_to_use"),
        arg_names: get_string_list(&mapping, "arguments"),
        allowed_tools: get_string_list(&mapping, "allowed-tools"),
        model: get_string(&mapping, "model"),
        context: get_string(&mapping, "context"),
        agent: get_string(&mapping, "agent"),
        effort: get_string(&mapping, "effort"),
        paths: get_paths(&mapping),
        hooks: get_json_value(&mapping, "hooks"),
        version: get_string(&mapping, "version"),
        user_invocable: get_bool(&mapping, "user-invocable"),
        disable_model_invocation: get_bool(&mapping, "disable-model-invocation"),
        argument_hint: get_string(&mapping, "argument-hint"),
    })
}

// ---------------------------------------------------------------------------
// YAML value extraction helpers
// ---------------------------------------------------------------------------

fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_string())
}

fn get_string(m: &serde_yaml::Mapping, key: &str) -> Option<String> {
    m.get(&yaml_key(key)).and_then(|v| match v {
        serde_yaml::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

fn get_bool(m: &serde_yaml::Mapping, key: &str) -> Option<bool> {
    m.get(&yaml_key(key)).and_then(|v| match v {
        serde_yaml::Value::Bool(b) => Some(*b),
        serde_yaml::Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

/// Extract a string list from either a YAML sequence or a comma-separated string.
fn get_string_list(m: &serde_yaml::Mapping, key: &str) -> Option<Vec<String>> {
    let val = m.get(&yaml_key(key))?;
    match val {
        serde_yaml::Value::Sequence(seq) => {
            let items: Vec<String> = seq
                .iter()
                .filter_map(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.trim().to_string()),
                    _ => None,
                })
                .filter(|s| !s.is_empty())
                .collect();
            if items.is_empty() {
                None
            } else {
                Some(items)
            }
        }
        serde_yaml::Value::String(s) => {
            let items: Vec<String> = s
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if items.is_empty() {
                None
            } else {
                Some(items)
            }
        }
        _ => None,
    }
}

/// Extract paths from frontmatter. Handles comma-separated strings and YAML
/// sequences, including brace expansion for glob patterns.
fn get_paths(m: &serde_yaml::Mapping) -> Option<Vec<String>> {
    let val = m.get(&yaml_key("paths"))?;
    let raw_items: Vec<String> = match val {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| match v {
                serde_yaml::Value::String(s) => Some(s.trim().to_string()),
                _ => None,
            })
            .collect(),
        serde_yaml::Value::String(s) => split_path_respecting_braces(s),
        _ => return None,
    };

    // Expand brace patterns and strip trailing `/**`.
    let patterns: Vec<String> = raw_items
        .into_iter()
        .flat_map(|p| expand_braces(&p))
        .map(|p| {
            if p.ends_with("/**") {
                p[..p.len() - 3].to_string()
            } else {
                p
            }
        })
        .filter(|p| !p.is_empty() && p != "**")
        .collect();

    if patterns.is_empty() {
        None
    } else {
        Some(patterns)
    }
}

/// Convert an arbitrary YAML value to a `serde_json::Value` for hooks storage.
fn get_json_value(m: &serde_yaml::Mapping, key: &str) -> Option<serde_json::Value> {
    let val = m.get(&yaml_key(key))?;
    yaml_to_json(val)
}

fn yaml_to_json(v: &serde_yaml::Value) -> Option<serde_json::Value> {
    match v {
        serde_yaml::Value::Null => Some(serde_json::Value::Null),
        serde_yaml::Value::Bool(b) => Some(serde_json::Value::Bool(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(serde_json::Value::Number(i.into()))
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f).map(serde_json::Value::Number)
            } else {
                None
            }
        }
        serde_yaml::Value::String(s) => Some(serde_json::Value::String(s.clone())),
        serde_yaml::Value::Sequence(seq) => {
            let arr: Vec<serde_json::Value> = seq.iter().filter_map(yaml_to_json).collect();
            Some(serde_json::Value::Array(arr))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in map {
                if let serde_yaml::Value::String(key) = k {
                    if let Some(jv) = yaml_to_json(val) {
                        obj.insert(key.clone(), jv);
                    }
                }
            }
            Some(serde_json::Value::Object(obj))
        }
        serde_yaml::Value::Tagged(t) => yaml_to_json(&t.value),
    }
}

// ---------------------------------------------------------------------------
// Brace expansion (for glob patterns in paths)
// ---------------------------------------------------------------------------

/// Split a comma-separated string while respecting brace groups.
fn split_path_respecting_braces(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut brace_depth: i32 = 0;

    for ch in input.chars() {
        match ch {
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth -= 1;
                current.push(ch);
            }
            ',' if brace_depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }
    parts
}

/// Expand brace patterns in a glob string.
///
/// ```text
/// expand_braces("src/*.{ts,tsx}") => ["src/*.ts", "src/*.tsx"]
/// expand_braces("{a,b}/{c,d}")    => ["a/c", "a/d", "b/c", "b/d"]
/// ```
fn expand_braces(pattern: &str) -> Vec<String> {
    // Find the first `{...}` group.
    let brace_re = Regex::new(r"^([^\{]*)\{([^\}]+)\}(.*)$").unwrap();
    if let Some(caps) = brace_re.captures(pattern) {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let alternatives = caps.get(2).map_or("", |m| m.as_str());
        let suffix = caps.get(3).map_or("", |m| m.as_str());

        alternatives
            .split(',')
            .flat_map(|alt| {
                let combined = format!("{prefix}{}{suffix}", alt.trim());
                expand_braces(&combined)
            })
            .collect()
    } else {
        vec![pattern.to_string()]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_content() {
        let (fm, body) = parse_skill_file("Hello world").unwrap();
        assert!(fm.name.is_none());
        assert_eq!(body, "Hello world");
    }

    #[test]
    fn test_parse_basic_frontmatter() {
        let input = r#"---
name: my-skill
description: Does something useful
---
# Body content
Some instructions here.
"#;
        let (fm, body) = parse_skill_file(input).unwrap();
        assert_eq!(fm.name.as_deref(), Some("my-skill"));
        assert_eq!(fm.description.as_deref(), Some("Does something useful"));
        assert!(body.starts_with("# Body content"));
    }

    #[test]
    fn test_parse_full_frontmatter() {
        let input = r#"---
name: code-review
description: Reviews code changes
when_to_use: When the user asks for a code review
allowed-tools: Read, Grep, Glob
model: sonnet
context: fork
agent: general-purpose
effort: high
version: "1.0"
user-invocable: true
disable-model-invocation: false
argument-hint: "<file path>"
---
Review the code.
"#;
        let (fm, body) = parse_skill_file(input).unwrap();
        assert_eq!(fm.name.as_deref(), Some("code-review"));
        assert_eq!(fm.description.as_deref(), Some("Reviews code changes"));
        assert_eq!(
            fm.when_to_use.as_deref(),
            Some("When the user asks for a code review")
        );
        assert_eq!(
            fm.allowed_tools.as_deref(),
            Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]).as_deref()
        );
        assert_eq!(fm.model.as_deref(), Some("sonnet"));
        assert_eq!(fm.context.as_deref(), Some("fork"));
        assert_eq!(fm.agent.as_deref(), Some("general-purpose"));
        assert_eq!(fm.effort.as_deref(), Some("high"));
        assert_eq!(fm.version.as_deref(), Some("1.0"));
        assert_eq!(fm.user_invocable, Some(true));
        assert_eq!(fm.disable_model_invocation, Some(false));
        assert_eq!(fm.argument_hint.as_deref(), Some("<file path>"));
        assert_eq!(body, "Review the code.\n");
    }

    #[test]
    fn test_parse_paths_as_list() {
        let input = r#"---
paths:
  - src/**/*.rs
  - tests/**/*.rs
---
content
"#;
        let (fm, _) = parse_skill_file(input).unwrap();
        let paths = fm.paths.unwrap();
        assert_eq!(paths, vec!["src/**/*.rs", "tests/**/*.rs"]);
    }

    #[test]
    fn test_parse_paths_as_comma_string() {
        let input = "---\npaths: src/**/*.rs, tests/**/*.rs\n---\ncontent\n";
        let (fm, _) = parse_skill_file(input).unwrap();
        let paths = fm.paths.unwrap();
        assert_eq!(paths, vec!["src/**/*.rs", "tests/**/*.rs"]);
    }

    #[test]
    fn test_brace_expansion_in_paths() {
        let input = "---\npaths: \"src/*.{ts,tsx}\"\n---\ncontent\n";
        let (fm, _) = parse_skill_file(input).unwrap();
        let paths = fm.paths.unwrap();
        assert_eq!(paths, vec!["src/*.ts", "src/*.tsx"]);
    }

    #[test]
    fn test_strip_double_star_suffix() {
        let input = "---\npaths: \"src/**\"\n---\ncontent\n";
        let (fm, _) = parse_skill_file(input).unwrap();
        let paths = fm.paths.unwrap();
        assert_eq!(paths, vec!["src"]);
    }

    #[test]
    fn test_all_star_paths_returns_none() {
        let input = "---\npaths: \"**\"\n---\ncontent\n";
        let (fm, _) = parse_skill_file(input).unwrap();
        assert!(fm.paths.is_none());
    }

    #[test]
    fn test_arguments_as_list() {
        let input = "---\narguments:\n  - file\n  - message\n---\ncontent\n";
        let (fm, _) = parse_skill_file(input).unwrap();
        assert_eq!(
            fm.arg_names.unwrap(),
            vec!["file".to_string(), "message".to_string()]
        );
    }

    #[test]
    fn test_hooks_parsed_as_json() {
        let input = r#"---
hooks:
  PreToolUse:
    - matcher: Bash
      hooks:
        - command: echo "pre"
---
content
"#;
        let (fm, _) = parse_skill_file(input).unwrap();
        let hooks = fm.hooks.unwrap();
        assert!(hooks.is_object());
        assert!(hooks.get("PreToolUse").is_some());
    }

    #[test]
    fn test_extract_description_fallback() {
        let content = "# Heading\n\nThis is the first paragraph.\n\nMore stuff.";
        let desc = extract_description_from_markdown(content, "Skill");
        assert_eq!(desc, "This is the first paragraph.");
    }

    #[test]
    fn test_quote_problematic_yaml() {
        let input = "allowed-tools: Read, **/*.{ts,tsx}";
        let quoted = quote_problematic_values(input);
        assert!(quoted.contains('"'));
    }

    #[test]
    fn test_no_frontmatter() {
        let (fm, body) = parse_skill_file("Just markdown content").unwrap();
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
        assert_eq!(body, "Just markdown content");
    }

    #[test]
    fn test_expand_braces() {
        assert_eq!(expand_braces("a"), vec!["a"]);
        assert_eq!(expand_braces("{a,b}"), vec!["a", "b"]);
        assert_eq!(
            expand_braces("src/*.{ts,tsx}"),
            vec!["src/*.ts", "src/*.tsx"]
        );
        assert_eq!(
            expand_braces("{a,b}/{c,d}"),
            vec!["a/c", "a/d", "b/c", "b/d"]
        );
    }

    #[test]
    fn test_split_path_respecting_braces() {
        let result = split_path_respecting_braces("a, src/*.{ts,tsx}, b");
        assert_eq!(result, vec!["a", "src/*.{ts,tsx}", "b"]);
    }
}
