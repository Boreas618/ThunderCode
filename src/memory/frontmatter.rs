//! YAML frontmatter parsing and serialization.
//!
//! Ported from ref/utils/frontmatterParser.ts`.
//!
//! Memory and RULES.md files use `---` delimited YAML frontmatter at the
//! top. This module parses that frontmatter and separates it from the body.

use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use crate::memory::types::MemoryFrontmatter;

// ---------------------------------------------------------------------------
// Regex
// ---------------------------------------------------------------------------

/// Matches the `---\n ... ---\n` frontmatter block at the start of a file.
static FRONTMATTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^---\s*\n([\s\S]*?)---\s*\n?").unwrap());

// ---------------------------------------------------------------------------
// Raw YAML frontmatter (loose deserialization)
// ---------------------------------------------------------------------------

/// Loosely-typed YAML frontmatter for initial parsing before validation.
#[derive(Debug, Default, serde::Deserialize)]
struct RawFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    memory_type: Option<String>,
    paths: Option<StringOrVec>,
    #[serde(rename = "allowed-tools")]
    #[allow(dead_code)]
    allowed_tools: Option<serde_yaml::Value>,
}

/// Handles YAML values that can be either a single string or a list.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

// ---------------------------------------------------------------------------
// Public API: Memory frontmatter
// ---------------------------------------------------------------------------

/// Parse YAML frontmatter from a memory file's content.
///
/// Returns the parsed [`MemoryFrontmatter`] and the body text after the
/// frontmatter block. If no frontmatter is found, returns default values
/// with the full content as the body.
pub fn parse_frontmatter(content: &str) -> Result<(MemoryFrontmatter, String)> {
    let (raw, body) = parse_raw_frontmatter(content);

    let fm = MemoryFrontmatter {
        name: raw.name.unwrap_or_default(),
        description: raw.description.unwrap_or_default(),
        memory_type: raw
            .memory_type
            .as_deref()
            .and_then(crate::memory::types::parse_memory_type),
    };

    Ok((fm, body))
}

/// Serialize a [`MemoryFrontmatter`] and body into a complete markdown file.
pub fn serialize_frontmatter(fm: &MemoryFrontmatter, body: &str) -> String {
    let mut out = String::with_capacity(256 + body.len());
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", fm.name));
    out.push_str(&format!(
        "description: {}\n",
        yaml_quote_if_needed(&fm.description)
    ));
    if let Some(ref ty) = fm.memory_type {
        out.push_str(&format!("type: {}\n", ty.as_str()));
    }
    out.push_str("---\n\n");
    out.push_str(body);
    out
}

// ---------------------------------------------------------------------------
// Public API: generic frontmatter (for RULES.md files)
// ---------------------------------------------------------------------------

/// Parsed result from a generic markdown file (RULES.md, rules, etc.).
#[derive(Debug)]
pub struct ParsedMarkdown {
    /// Description from frontmatter, if any.
    pub description: Option<String>,
    /// Glob patterns from `paths:` frontmatter, if any.
    pub paths: Option<Vec<String>>,
    /// Body content after the frontmatter.
    pub content: String,
}

/// Parse generic frontmatter from a markdown file (RULES.md, rules, etc.).
///
/// Extracts `description` and `paths` fields, returns the body content.
pub fn parse_generic_frontmatter(content: &str) -> ParsedMarkdown {
    let (raw, body) = parse_raw_frontmatter(content);

    let paths = raw.paths.map(|p| match p {
        StringOrVec::Single(s) => split_path_patterns(&s),
        StringOrVec::Multiple(v) => v
            .iter()
            .flat_map(|s| split_path_patterns(s))
            .collect(),
    });

    ParsedMarkdown {
        description: raw.description,
        paths,
        content: body,
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Parse the raw YAML frontmatter block and separate it from the body.
fn parse_raw_frontmatter(content: &str) -> (RawFrontmatter, String) {
    let Some(captures) = FRONTMATTER_REGEX.captures(content) else {
        return (RawFrontmatter::default(), content.to_string());
    };

    let frontmatter_text = captures.get(1).map_or("", |m| m.as_str());
    let full_match_len = captures.get(0).unwrap().as_str().len();
    let body = content[full_match_len..].to_string();

    // Try parsing the raw YAML. On failure, try quoting problematic values.
    let raw = match serde_yaml::from_str::<RawFrontmatter>(frontmatter_text) {
        Ok(r) => r,
        Err(_) => {
            let quoted = quote_problematic_values(frontmatter_text);
            serde_yaml::from_str::<RawFrontmatter>(&quoted).unwrap_or_default()
        }
    };

    (raw, body)
}

/// Quote YAML values that contain special characters (glob braces, anchors, etc.).
///
/// Ported from `quoteProblematicValues` in frontmatterParser.ts.
fn quote_problematic_values(text: &str) -> String {
    static YAML_SPECIAL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"[{}\[\]*&#!|>%@`]|: "#).unwrap());
    static KEY_VALUE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([a-zA-Z_-]+):\s+(.+)$").unwrap());

    let mut result = Vec::new();
    for line in text.lines() {
        if let Some(caps) = KEY_VALUE.captures(line) {
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();

            // Already quoted?
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                result.push(line.to_string());
                continue;
            }

            if YAML_SPECIAL.is_match(value) {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                result.push(format!("{key}: \"{escaped}\""));
                continue;
            }
        }
        result.push(line.to_string());
    }
    result.join("\n")
}

/// Quote a string for YAML output if it contains characters that need quoting.
fn yaml_quote_if_needed(s: &str) -> String {
    if s.contains(':') || s.contains('#') || s.contains('"') || s.contains('\'') {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// Split comma-separated path patterns, respecting brace groups.
///
/// Ported from `splitPathInFrontmatter` in frontmatterParser.ts.
fn split_path_patterns(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0u32;

    for ch in input.chars() {
        match ch {
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::MemoryType;

    #[test]
    fn parse_valid_frontmatter() {
        let input = "---\nname: test memory\ndescription: a test\ntype: user\n---\n\nBody text here.\n";
        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.name, "test memory");
        assert_eq!(fm.description, "a test");
        assert_eq!(fm.memory_type, Some(MemoryType::User));
        assert!(body.trim().contains("Body text here."));
    }

    #[test]
    fn parse_no_frontmatter() {
        let input = "Just plain markdown content.\n";
        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.name, "");
        assert_eq!(fm.description, "");
        assert_eq!(fm.memory_type, None);
        assert_eq!(body, input);
    }

    #[test]
    fn parse_frontmatter_no_type() {
        let input = "---\nname: legacy\ndescription: old file\n---\n\nBody.\n";
        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.name, "legacy");
        assert_eq!(fm.memory_type, None);
        assert!(body.contains("Body."));
    }

    #[test]
    fn parse_frontmatter_unknown_type() {
        let input = "---\nname: x\ndescription: y\ntype: unknown_type\n---\n\nBody.\n";
        let (fm, _body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.memory_type, None);
    }

    #[test]
    fn serialize_round_trip() {
        let fm = MemoryFrontmatter {
            name: "test".to_string(),
            description: "a test memory".to_string(),
            memory_type: Some(MemoryType::Feedback),
        };
        let body = "This is the memory content.\n";
        let serialized = serialize_frontmatter(&fm, body);

        assert!(serialized.starts_with("---\n"));
        assert!(serialized.contains("name: test"));
        assert!(serialized.contains("description: a test memory"));
        assert!(serialized.contains("type: feedback"));
        assert!(serialized.ends_with("This is the memory content.\n"));

        // Re-parse and verify
        let (fm2, body2) = parse_frontmatter(&serialized).unwrap();
        assert_eq!(fm2.name, fm.name);
        assert_eq!(fm2.description, fm.description);
        assert_eq!(fm2.memory_type, fm.memory_type);
        assert!(body2.trim().contains("This is the memory content."));
    }

    #[test]
    fn split_path_patterns_basic() {
        let patterns = split_path_patterns("a, b, c");
        assert_eq!(patterns, vec!["a", "b", "c"]);
    }

    #[test]
    fn split_path_patterns_braces() {
        let patterns = split_path_patterns("a, src/*.{ts,tsx}");
        assert_eq!(patterns, vec!["a", "src/*.{ts,tsx}"]);
    }

    #[test]
    fn generic_frontmatter_with_paths() {
        let input = "---\ndescription: a rule\npaths: \"src/**/*.rs, tests/**\"\n---\n\nRule content.\n";
        let parsed = parse_generic_frontmatter(input);
        assert_eq!(parsed.description.as_deref(), Some("a rule"));
        assert!(parsed.paths.is_some());
        let paths = parsed.paths.unwrap();
        assert!(paths.contains(&"src/**/*.rs".to_string()));
        assert!(paths.contains(&"tests/**".to_string()));
        assert!(parsed.content.contains("Rule content."));
    }

    #[test]
    fn quote_problematic_values_glob() {
        let input = "paths: **/*.{ts,tsx}";
        let result = quote_problematic_values(input);
        assert!(result.contains('"'));
    }

    #[test]
    fn description_with_colon_is_quoted() {
        let fm = MemoryFrontmatter {
            name: "test".to_string(),
            description: "key: value pair".to_string(),
            memory_type: None,
        };
        let serialized = serialize_frontmatter(&fm, "body");
        assert!(serialized.contains("description: \"key: value pair\""));
    }
}
