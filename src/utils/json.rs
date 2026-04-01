//! JSON utilities: safe parsing, NDJSON/JSONL parsing, partial JSON repair.
//!
//! Ported from ref/utils/json.ts`. Uses `serde_json` throughout.

use serde::de::DeserializeOwned;

/// Safely parse a JSON string, returning `None` on any parse error.
///
/// # Examples
/// ```
/// use crate::utils::json::safe_parse_json;
/// let val: Option<serde_json::Value> = safe_parse_json(r#"{"a": 1}"#);
/// assert!(val.is_some());
/// let bad: Option<serde_json::Value> = safe_parse_json("not json");
/// assert!(bad.is_none());
/// ```
pub fn safe_parse_json<T: DeserializeOwned>(json: &str) -> Option<T> {
    let cleaned = strip_bom(json);
    serde_json::from_str(cleaned).ok()
}

/// Safely parse a JSON string into a [`serde_json::Value`], returning `None` on error.
pub fn safe_parse_json_value(json: &str) -> Option<serde_json::Value> {
    safe_parse_json(json)
}

/// Strip UTF-8 BOM (byte order mark) from the beginning of a string.
///
/// PowerShell 5.x and some editors prepend a BOM to UTF-8 files.
pub fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

/// Parse NDJSON/JSONL data, skipping malformed lines.
///
/// Each line is parsed independently; blank lines and invalid JSON lines are
/// silently skipped (matching the TypeScript behaviour).
///
/// # Examples
/// ```
/// use crate::utils::json::parse_jsonl;
/// let data = "{\"a\":1}\n{\"b\":2}\nbad\n{\"c\":3}";
/// let values: Vec<serde_json::Value> = parse_jsonl(data);
/// assert_eq!(values.len(), 3);
/// ```
pub fn parse_jsonl<T: DeserializeOwned>(data: &str) -> Vec<T> {
    let cleaned = strip_bom(data);
    cleaned
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str(trimmed).ok()
        })
        .collect()
}

/// Parse JSONL from bytes, handling BOM and skipping malformed lines.
pub fn parse_jsonl_bytes<T: DeserializeOwned>(data: &[u8]) -> Vec<T> {
    // Strip UTF-8 BOM (EF BB BF)
    let data = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        &data[3..]
    } else {
        data
    };

    match std::str::from_utf8(data) {
        Ok(s) => parse_jsonl(s),
        Err(_) => Vec::new(),
    }
}

/// Attempt to repair partial/truncated JSON by closing open brackets and braces.
///
/// This is a best-effort heuristic for streaming scenarios where a JSON value
/// may be truncated mid-way. It closes any unmatched `[` or `{` and terminates
/// unterminated strings.
///
/// # Examples
/// ```
/// use crate::utils::json::repair_partial_json;
/// let repaired = repair_partial_json(r#"{"key": "val"#);
/// assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
/// ```
pub fn repair_partial_json(partial: &str) -> String {
    let mut result = partial.to_string();
    let mut open_braces = 0i32;
    let mut open_brackets = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for c in partial.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match c {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '{' if !in_string => {
                open_braces += 1;
            }
            '}' if !in_string => {
                open_braces -= 1;
            }
            '[' if !in_string => {
                open_brackets += 1;
            }
            ']' if !in_string => {
                open_brackets -= 1;
            }
            _ => {}
        }
    }

    // Close unterminated string
    if in_string {
        result.push('"');
    }

    // Close open brackets and braces (innermost first)
    // We track the order of opens to close in reverse order
    // Simplified: close brackets first, then braces (rough heuristic)
    for _ in 0..open_brackets {
        result.push(']');
    }
    for _ in 0..open_braces {
        result.push('}');
    }

    result
}

/// Serialize a value to a pretty-printed JSON string.
pub fn to_json_pretty<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Serialize a value to a compact JSON string.
pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_safe_parse_json_valid() {
        let val: Option<serde_json::Value> = safe_parse_json(r#"{"key": "value"}"#);
        assert_eq!(val, Some(json!({"key": "value"})));
    }

    #[test]
    fn test_safe_parse_json_invalid() {
        let val: Option<serde_json::Value> = safe_parse_json("not json at all");
        assert_eq!(val, None);
    }

    #[test]
    fn test_safe_parse_json_null() {
        let val: Option<serde_json::Value> = safe_parse_json("null");
        assert_eq!(val, Some(serde_json::Value::Null));
    }

    #[test]
    fn test_safe_parse_json_empty() {
        let val: Option<serde_json::Value> = safe_parse_json("");
        assert_eq!(val, None);
    }

    #[test]
    fn test_strip_bom() {
        assert_eq!(strip_bom("\u{FEFF}{\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_bom("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn test_parse_jsonl() {
        let data = "{\"a\":1}\n{\"b\":2}\n\nbad line\n{\"c\":3}\n";
        let values: Vec<serde_json::Value> = parse_jsonl(data);
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], json!({"a": 1}));
        assert_eq!(values[1], json!({"b": 2}));
        assert_eq!(values[2], json!({"c": 3}));
    }

    #[test]
    fn test_parse_jsonl_empty() {
        let values: Vec<serde_json::Value> = parse_jsonl("");
        assert!(values.is_empty());
    }

    #[test]
    fn test_parse_jsonl_with_bom() {
        let data = "\u{FEFF}{\"a\":1}\n{\"b\":2}";
        let values: Vec<serde_json::Value> = parse_jsonl(data);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_repair_partial_json_complete() {
        let repaired = repair_partial_json(r#"{"key": "value"}"#);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_repair_partial_json_unterminated_string() {
        let repaired = repair_partial_json(r#"{"key": "val"#);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_repair_partial_json_open_brace() {
        let repaired = repair_partial_json(r#"{"key": "value""#);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_repair_partial_json_open_bracket() {
        let repaired = repair_partial_json(r#"[1, 2, 3"#);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_parse_jsonl_bytes() {
        let data = b"{\"a\":1}\n{\"b\":2}";
        let values: Vec<serde_json::Value> = parse_jsonl_bytes(data);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_parse_jsonl_bytes_with_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"{\"a\":1}\n{\"b\":2}");
        let values: Vec<serde_json::Value> = parse_jsonl_bytes(&data);
        assert_eq!(values.len(), 2);
    }
}
