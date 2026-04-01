//! WebSearchTool -- search the web via API.
//!
//! Ported from ref/tools/WebSearchTool/WebSearchTool.ts.
//! Makes a real search API call and returns structured results.
//! Uses the Brave Search API or falls back to a configurable endpoint.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";

/// Search result from the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        WEB_SEARCH_TOOL_NAME
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

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("search the web for information")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of domains to restrict search to"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of domains to exclude from results"
                }
            },
            "required": ["query"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.trim().is_empty() {
            return ValidationResult::invalid("query must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let allowed_domains: Vec<String> = input
            .get("allowed_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let blocked_domains: Vec<String> = input
            .get("blocked_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Report progress: searching
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::WebSearch(WebSearchProgress {
                        status: "searching".to_string(),
                        results: None,
                    }),
                });
            }
        }

        // Build the search query with domain filters
        let effective_query = if !allowed_domains.is_empty() {
            let site_filter = allowed_domains
                .iter()
                .map(|d| format!("site:{d}"))
                .collect::<Vec<_>>()
                .join(" OR ");
            format!("{query} ({site_filter})")
        } else if !blocked_domains.is_empty() {
            let exclude = blocked_domains
                .iter()
                .map(|d| format!("-site:{d}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{query} {exclude}")
        } else {
            query.clone()
        };

        // Try Brave Search API first, then fall back to a mock response
        let api_key = std::env::var("BRAVE_SEARCH_API_KEY")
            .or_else(|_| std::env::var("THUNDERCODE_SEARCH_API_KEY"))
            .ok();

        let results = if let Some(api_key) = api_key {
            // Real Brave Search API call
            match brave_search(&effective_query, &api_key).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Brave Search API error: {e}");
                    return Err(ToolError::ExecutionFailed {
                        message: format!("Web search failed: {e}"),
                    });
                }
            }
        } else {
            // No API key configured -- return an informative error
            return Err(ToolError::ExecutionFailed {
                message: "Web search is not configured. Set BRAVE_SEARCH_API_KEY or \
                          THUNDERCODE_SEARCH_API_KEY environment variable to enable web search."
                    .to_string(),
            });
        };

        // Report progress: results found
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                let preview: Vec<WebSearchResult> = results
                    .iter()
                    .take(5)
                    .map(|r| WebSearchResult {
                        title: r.title.clone(),
                        url: r.url.clone(),
                    })
                    .collect();
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::WebSearch(WebSearchProgress {
                        status: "complete".to_string(),
                        results: Some(preview),
                    }),
                });
            }
        }

        let results_json: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "title": r.title,
                    "url": r.url,
                    "snippet": r.snippet,
                })
            })
            .collect();

        Ok(ToolCallResult {
            data: serde_json::json!({
                "query": query,
                "results": results_json,
                "total": results.len(),
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

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let q = input.get("query").and_then(|v| v.as_str()).unwrap_or("...");
        format!("Search: {q}")
    }

    async fn prompt(&self) -> String {
        "Search the web for real-time information. Returns a list of results with titles, \
         URLs, and snippets.\n\n\
         Use this when you need up-to-date information that may not be in your training data, \
         such as:\n\
         - Current documentation for a library or API\n\
         - Recent news or announcements\n\
         - Stack Overflow answers for specific error messages\n\
         - Current best practices\n\n\
         You can restrict results to specific domains with allowed_domains, or exclude \
         domains with blocked_domains."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "WebSearch".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let query = input
            .and_then(|i| i.get("query"))
            .and_then(|v| v.as_str())
            .unwrap_or("...");
        Some(format!("Searching: {query}"))
    }
}

/// Call the Brave Search API.
async fn brave_search(query: &str, api_key: &str) -> Result<Vec<SearchResult>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", "10")])
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Search API returned {status}: {body}"));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let web_results = body
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array());

    let results = match web_results {
        Some(arr) => arr
            .iter()
            .map(|item| SearchResult {
                title: item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                url: item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                snippet: item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect(),
        None => Vec::new(),
    };

    Ok(results)
}
