//! WebFetchTool -- HTTP GET with HTML-to-text conversion.
//!
//! Ported from ref/tools/WebFetchTool/WebFetchTool.ts.
//! Fetches a URL and converts HTML to readable text/markdown.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use serde::{Deserialize, Serialize};

pub const WEB_FETCH_TOOL_NAME: &str = "WebFetch";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchInput {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        WEB_FETCH_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("fetch web page content, HTTP GET")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional instruction for content extraction (e.g., 'Extract the API documentation')"
                }
            },
            "required": ["url"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            return ValidationResult::invalid("url must not be empty", 9);
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ValidationResult::invalid("url must start with http:// or https://", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let fetch_input: WebFetchInput = serde_json::from_value(input).map_err(|e| {
            ToolError::ValidationFailed {
                message: format!("Invalid input: {e}"),
            }
        })?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("ThunderCode/0.1 (compatible; bot)")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("HTTP client error: {e}"),
            })?;

        let response = client
            .get(&fetch_input.url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Fetch failed: {e}"),
            })?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed {
                message: format!("HTTP {} for {}", status, fetch_input.url),
            });
        }

        let body = response.text().await.map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to read response: {e}"),
        })?;

        // Convert HTML to readable text
        let text = if content_type.contains("text/html") || content_type.contains("application/xhtml") {
            html_to_text(&body)
        } else {
            body
        };

        // Truncate to a reasonable size
        let max_chars = 80_000;
        let (truncated, was_truncated) = if text.len() > max_chars {
            (text[..max_chars].to_string(), true)
        } else {
            (text, false)
        };

        let mut data = serde_json::json!({
            "url": fetch_input.url,
            "status": status,
            "contentType": content_type,
            "content": truncated,
        });

        if was_truncated {
            data["truncated"] = serde_json::json!(true);
            data["totalLength"] = serde_json::json!(truncated.len() + max_chars);
        }

        Ok(ToolCallResult {
            data,
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _ctx: &ToolPermissionContext) -> String {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");
        format!("Fetch {url}")
    }

    async fn prompt(&self) -> String {
        "Fetch the content of a web page and convert it to readable text.\n\
         HTML is automatically converted to a text representation.\n\n\
         Use this when you need to read the content of a specific URL, such as:\n\
         - Documentation pages\n\
         - GitHub issues or PRs\n\
         - Blog posts or articles\n\
         - API references"
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "WebFetch".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let url = input
            .and_then(|i| i.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("...");
        Some(format!("Fetching: {url}"))
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchReadInfo {
        SearchReadInfo {
            is_search: false,
            is_read: true,
            is_list: None,
        }
    }
}

/// Convert HTML to readable text.
/// Handles common HTML elements and produces a clean text representation.
fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut in_script = false;
    let mut in_style = false;
    let mut last_was_newline = false;
    let mut in_pre = false;
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '<' {
            in_tag = true;
            tag_name.clear();
            i += 1;
            continue;
        }

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_name.to_lowercase();
                let is_closing = tag_lower.starts_with('/');
                let bare_tag = if is_closing {
                    tag_lower[1..].split_whitespace().next().unwrap_or("")
                } else {
                    tag_lower.split_whitespace().next().unwrap_or("")
                };

                match bare_tag {
                    "script" => in_script = !is_closing,
                    "style" => in_style = !is_closing,
                    "pre" | "code" => in_pre = !is_closing,
                    _ => {}
                }

                // Add newlines for block elements
                if !is_closing {
                    match bare_tag {
                        "br" | "hr" => {
                            result.push('\n');
                            last_was_newline = true;
                        }
                        "p" | "div" | "section" | "article" | "header" | "footer"
                        | "nav" | "main" | "aside" | "blockquote" | "figure"
                        | "figcaption" | "details" | "summary" => {
                            if !last_was_newline {
                                result.push('\n');
                                last_was_newline = true;
                            }
                        }
                        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                            if !last_was_newline {
                                result.push('\n');
                            }
                            result.push('\n');
                            // Add markdown-style heading prefix
                            let level = bare_tag.chars().nth(1).unwrap_or('1');
                            let hashes = "#".repeat(
                                level.to_digit(10).unwrap_or(1) as usize,
                            );
                            result.push_str(&hashes);
                            result.push(' ');
                            last_was_newline = false;
                        }
                        "li" => {
                            if !last_was_newline {
                                result.push('\n');
                            }
                            result.push_str("- ");
                            last_was_newline = false;
                        }
                        "tr" => {
                            if !last_was_newline {
                                result.push('\n');
                            }
                            last_was_newline = true;
                        }
                        "td" | "th" => {
                            result.push_str(" | ");
                            last_was_newline = false;
                        }
                        _ => {}
                    }
                } else {
                    // Closing tags
                    match bare_tag {
                        "p" | "div" | "section" | "article" | "blockquote"
                        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                        | "ul" | "ol" | "table" => {
                            if !last_was_newline {
                                result.push('\n');
                                last_was_newline = true;
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                tag_name.push(ch);
            }
            i += 1;
            continue;
        }

        // Skip content inside <script> and <style> tags
        if in_script || in_style {
            i += 1;
            continue;
        }

        // Decode HTML entities
        if ch == '&' {
            // Look for the entity end
            let rest: String = chars[i..].iter().take(10).collect();
            if let Some(end) = rest.find(';') {
                let entity = &rest[..=end];
                let decoded = match entity {
                    "&amp;" => "&",
                    "&lt;" => "<",
                    "&gt;" => ">",
                    "&quot;" => "\"",
                    "&apos;" | "&#39;" => "'",
                    "&nbsp;" | "&#160;" => " ",
                    "&mdash;" | "&#8212;" => "--",
                    "&ndash;" | "&#8211;" => "-",
                    "&hellip;" | "&#8230;" => "...",
                    "&copy;" => "(c)",
                    "&reg;" => "(R)",
                    "&trade;" => "(TM)",
                    _ => {
                        // Try numeric entity
                        if entity.starts_with("&#x") {
                            let hex = &entity[3..entity.len() - 1];
                            if let Ok(code) = u32::from_str_radix(hex, 16) {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                    i += end + 1;
                                    last_was_newline = false;
                                    continue;
                                }
                            }
                        } else if entity.starts_with("&#") {
                            let num = &entity[2..entity.len() - 1];
                            if let Ok(code) = num.parse::<u32>() {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                    i += end + 1;
                                    last_was_newline = false;
                                    continue;
                                }
                            }
                        }
                        // Unknown entity, output as-is
                        result.push_str(entity);
                        i += end + 1;
                        last_was_newline = false;
                        continue;
                    }
                };
                result.push_str(decoded);
                i += end + 1;
                last_was_newline = false;
                continue;
            }
        }

        // Handle whitespace
        if !in_pre && (ch == '\n' || ch == '\r' || ch == '\t') {
            if !last_was_newline && !result.ends_with(' ') {
                result.push(' ');
            }
            i += 1;
            continue;
        }

        // Collapse multiple spaces (outside <pre>)
        if !in_pre && ch == ' ' && result.ends_with(' ') {
            i += 1;
            continue;
        }

        result.push(ch);
        last_was_newline = ch == '\n';
        i += 1;
    }

    // Clean up: collapse multiple blank lines
    let mut cleaned = String::with_capacity(result.len());
    let mut blank_count = 0;
    for line in result.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                cleaned.push('\n');
            }
        } else {
            blank_count = 0;
            cleaned.push_str(line.trim_end());
            cleaned.push('\n');
        }
    }

    cleaned.trim().to_string()
}
