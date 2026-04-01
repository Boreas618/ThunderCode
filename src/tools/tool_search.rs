//! ToolSearchTool -- search for and load deferred tools.
//!
//! Ported from ref/tools/ToolSearchTool/ToolSearchTool.ts.
//! Implements keyword-based search over tool names, descriptions, and
//! searchHints. Supports both `select:Name,Name` direct selection and
//! freeform keyword queries with `+required` term syntax.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const TOOL_SEARCH_TOOL_NAME: &str = "ToolSearch";

pub struct ToolSearchTool;

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a tool name into searchable parts.
/// Handles MCP tools (mcp__server__action) and regular CamelCase tools.
struct ParsedToolName {
    parts: Vec<String>,
    full: String,
    is_mcp: bool,
}

fn parse_tool_name(name: &str) -> ParsedToolName {
    if name.starts_with("mcp__") {
        let without_prefix = name.strip_prefix("mcp__").unwrap_or(name).to_lowercase();
        let parts: Vec<String> = without_prefix
            .split("__")
            .flat_map(|p| p.split('_'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let full = without_prefix.replace("__", " ").replace('_', " ");
        ParsedToolName {
            parts,
            full,
            is_mcp: true,
        }
    } else {
        // Split CamelCase: insert space before uppercase letters following lowercase
        let mut spaced = String::with_capacity(name.len() + 8);
        let chars: Vec<char> = name.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if i > 0 && ch.is_uppercase() && i > 0 && chars[i - 1].is_lowercase() {
                spaced.push(' ');
            }
            spaced.push(ch);
        }
        let lower = spaced.replace('_', " ").to_lowercase();
        let parts: Vec<String> = lower
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        ParsedToolName {
            full: parts.join(" "),
            parts,
            is_mcp: false,
        }
    }
}

/// Check if a word appears at a word boundary in text.
fn word_boundary_match(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    // Simple word boundary check: look for the word surrounded by non-alphanumeric chars
    let lower = text.to_lowercase();
    let w = word.to_lowercase();
    for (idx, _) in lower.match_indices(&w) {
        let before_ok = idx == 0
            || !lower.as_bytes()[idx - 1].is_ascii_alphanumeric();
        let after_idx = idx + w.len();
        let after_ok = after_idx >= lower.len()
            || !lower.as_bytes()[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Score a tool against search terms. Returns 0 if no match.
fn score_tool(
    parsed: &ParsedToolName,
    hint: Option<&str>,
    description: &str,
    terms: &[String],
) -> u32 {
    let hint_lower = hint.unwrap_or("").to_lowercase();
    let desc_lower = description.to_lowercase();
    let mut score: u32 = 0;

    for term in terms {
        // Exact part match (high weight for MCP server names, tool name parts)
        if parsed.parts.iter().any(|p| p == term) {
            score += if parsed.is_mcp { 12 } else { 10 };
        } else if parsed.parts.iter().any(|p| p.contains(term.as_str())) {
            score += if parsed.is_mcp { 6 } else { 5 };
        }

        // Full name fallback
        if parsed.full.contains(term.as_str()) && score == 0 {
            score += 3;
        }

        // searchHint match — curated capability phrase, higher signal
        if !hint_lower.is_empty() && word_boundary_match(&hint_lower, term) {
            score += 4;
        }

        // Description match with word boundary
        if word_boundary_match(&desc_lower, term) {
            score += 2;
        }
    }

    score
}

// ---------------------------------------------------------------------------
// Data structures used to pass tool info for searching
// ---------------------------------------------------------------------------

/// Minimal tool info for search purposes (avoids needing the full Tool trait objects).
/// In production use, the caller passes these from the registry.
#[derive(Debug, Clone)]
pub struct DeferredToolInfo {
    pub name: String,
    pub search_hint: Option<String>,
    pub description: String,
    pub is_mcp: bool,
    pub should_defer: bool,
    pub always_load: bool,
}

/// Check if a tool should be deferred (matches ref isDeferredTool logic).
pub fn is_deferred_tool(tool: &dyn Tool) -> bool {
    // Explicit opt-out
    if tool.always_load() {
        return false;
    }
    // MCP tools are always deferred
    if tool.is_mcp() {
        return true;
    }
    // Never defer ToolSearch itself
    if tool.name() == TOOL_SEARCH_TOOL_NAME {
        return false;
    }
    tool.should_defer()
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        TOOL_SEARCH_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn always_load(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("search for available tools by keyword")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Query to find deferred tools. Use \"select:<tool_name>\" for direct selection, or keywords to search."
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 5)",
                    "default": 5
                }
            },
            "required": ["query", "max_results"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // Build the list of deferred tools from all base tools.
        // In the real system, the tool list would come from the context/options.
        // For now, we use the static registry.
        let all_tools = crate::tools::registry::ToolRegistry::get_all_base_tools();
        let deferred_tools: Vec<DeferredToolInfo> = all_tools
            .iter()
            .filter(|t| is_deferred_tool(t.as_ref()))
            .map(|t| {
                DeferredToolInfo {
                    name: t.name().to_string(),
                    search_hint: t.search_hint().map(|s| s.to_string()),
                    description: String::new(), // Will be populated lazily
                    is_mcp: t.is_mcp(),
                    should_defer: t.should_defer(),
                    always_load: t.always_load(),
                }
            })
            .collect();

        let all_tool_infos: Vec<DeferredToolInfo> = all_tools
            .iter()
            .map(|t| {
                DeferredToolInfo {
                    name: t.name().to_string(),
                    search_hint: t.search_hint().map(|s| s.to_string()),
                    description: String::new(),
                    is_mcp: t.is_mcp(),
                    should_defer: t.should_defer(),
                    always_load: t.always_load(),
                }
            })
            .collect();

        let total_deferred = deferred_tools.len();

        // Check for select: prefix -- direct tool selection
        if let Some(after) = query.strip_prefix("select:").or_else(|| query.strip_prefix("Select:")) {
            let requested: Vec<&str> = after.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let mut found: Vec<String> = Vec::new();

            for tool_name in &requested {
                // Check deferred tools first, then all tools
                let matched = deferred_tools
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(tool_name))
                    .or_else(|| all_tool_infos.iter().find(|t| t.name.eq_ignore_ascii_case(tool_name)));

                if let Some(t) = matched {
                    if !found.contains(&t.name) {
                        found.push(t.name.clone());
                    }
                }
            }

            return Ok(ToolCallResult {
                data: serde_json::json!({
                    "matches": found,
                    "query": query,
                    "total_deferred_tools": total_deferred,
                }),
                new_messages: None,
                mcp_meta: None,
            });
        }

        // Keyword search
        let query_lower = query.to_lowercase().trim().to_string();

        // Fast path: exact tool name match
        let exact_match = deferred_tools
            .iter()
            .find(|t| t.name.to_lowercase() == query_lower)
            .or_else(|| all_tool_infos.iter().find(|t| t.name.to_lowercase() == query_lower));
        if let Some(m) = exact_match {
            return Ok(ToolCallResult {
                data: serde_json::json!({
                    "matches": [m.name],
                    "query": query,
                    "total_deferred_tools": total_deferred,
                }),
                new_messages: None,
                mcp_meta: None,
            });
        }

        // MCP prefix match
        if query_lower.starts_with("mcp__") && query_lower.len() > 5 {
            let prefix_matches: Vec<String> = deferred_tools
                .iter()
                .filter(|t| t.name.to_lowercase().starts_with(&query_lower))
                .take(max_results)
                .map(|t| t.name.clone())
                .collect();
            if !prefix_matches.is_empty() {
                return Ok(ToolCallResult {
                    data: serde_json::json!({
                        "matches": prefix_matches,
                        "query": query,
                        "total_deferred_tools": total_deferred,
                    }),
                    new_messages: None,
                    mcp_meta: None,
                });
            }
        }

        // Parse query into required (+prefix) and optional terms
        let query_terms: Vec<&str> = query_lower.split_whitespace().filter(|t| !t.is_empty()).collect();
        let mut required_terms: Vec<String> = Vec::new();
        let mut optional_terms: Vec<String> = Vec::new();
        for term in &query_terms {
            if let Some(stripped) = term.strip_prefix('+') {
                if !stripped.is_empty() {
                    required_terms.push(stripped.to_string());
                }
            } else {
                optional_terms.push(term.to_string());
            }
        }

        let all_scoring_terms: Vec<String> = if !required_terms.is_empty() {
            required_terms.iter().chain(optional_terms.iter()).cloned().collect()
        } else {
            query_terms.iter().map(|t| t.to_string()).collect()
        };

        // Pre-filter by required terms
        let candidates: Vec<&DeferredToolInfo> = if !required_terms.is_empty() {
            deferred_tools
                .iter()
                .filter(|tool| {
                    let parsed = parse_tool_name(&tool.name);
                    let hint_lower = tool.search_hint.as_deref().unwrap_or("").to_lowercase();
                    required_terms.iter().all(|term| {
                        parsed.parts.iter().any(|p| p == term)
                            || parsed.parts.iter().any(|p| p.contains(term.as_str()))
                            || word_boundary_match(&hint_lower, term)
                    })
                })
                .collect()
        } else {
            deferred_tools.iter().collect()
        };

        // Score all candidates
        let mut scored: Vec<(&str, u32)> = candidates
            .iter()
            .map(|tool| {
                let parsed = parse_tool_name(&tool.name);
                let s = score_tool(
                    &parsed,
                    tool.search_hint.as_deref(),
                    &tool.description,
                    &all_scoring_terms,
                );
                (tool.name.as_str(), s)
            })
            .filter(|(_, s)| *s > 0)
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        let matches: Vec<String> = scored
            .into_iter()
            .take(max_results)
            .map(|(name, _)| name.to_string())
            .collect();

        Ok(ToolCallResult {
            data: serde_json::json!({
                "matches": matches,
                "query": query,
                "total_deferred_tools": total_deferred,
            }),
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, _: &serde_json::Value, _: &ToolPermissionContext) -> String {
        "Search for available tools".to_string()
    }

    async fn prompt(&self) -> String {
        "Fetches full schema definitions for deferred tools so they can be called.\n\n\
         Deferred tools appear by name in <system-reminder> messages. Until fetched, only the name \
         is known -- there is no parameter schema, so the tool cannot be invoked. This tool takes a \
         query, matches it against the deferred tool list, and returns the matched tools' complete \
         JSONSchema definitions inside a <functions> block. Once a tool's schema appears in that \
         result, it is callable exactly like any tool defined at the top of the prompt.\n\n\
         Result format: each matched tool appears as one <function>{\"description\": \"...\", \
         \"name\": \"...\", \"parameters\": {...}}</function> line inside the <functions> block -- \
         the same encoding as the tool list at the top of this prompt.\n\n\
         Query forms:\n\
         - \"select:Read,Edit,Grep\" -- fetch these exact tools by name\n\
         - \"notebook jupyter\" -- keyword search, up to max_results best matches\n\
         - \"+slack send\" -- require \"slack\" in the name, rank by remaining terms"
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }
}
