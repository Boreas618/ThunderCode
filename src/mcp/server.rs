//! MCP server: expose ThunderCode tools as an MCP server.
//!
//! This allows other MCP clients to discover and invoke ThunderCode's
//! built-in tools through the MCP protocol.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::mcp::jsonrpc::{self, error_codes, JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::mcp::types::McpToolDescription;

// ============================================================================
// McpServer
// ============================================================================

/// An MCP server that exposes tools to external clients.
///
/// Runs on stdio by default: reads JSON-RPC requests from stdin and writes
/// responses to stdout.
pub struct McpServer {
    pub name: String,
    pub version: String,
    pub tools: Vec<McpToolDescription>,
    /// Tool handler: given a tool name and arguments, returns the result.
    tool_handler:
        Option<Arc<dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync>>,
}

impl McpServer {
    /// Create a new MCP server with the given identity and tools.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        tools: Vec<McpToolDescription>,
    ) -> Self {
        McpServer {
            name: name.into(),
            version: version.into(),
            tools,
            tool_handler: None,
        }
    }

    /// Set the tool handler function.
    pub fn with_tool_handler(
        mut self,
        handler: impl Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
    ) -> Self {
        self.tool_handler = Some(Arc::new(handler));
        self
    }

    /// Handle a single JSON-RPC request and produce a response.
    pub fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request),
            "resources/list" => self.handle_resources_list(request),
            "ping" => JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({}),
            ),
            _ => {
                // Check for notifications (which we can silently ignore).
                if request.method.starts_with("notifications/") {
                    JsonRpcResponse::success(request.id.clone(), serde_json::json!({}))
                } else {
                    JsonRpcResponse::error(
                        request.id.clone(),
                        JsonRpcError::method_not_found(Some(serde_json::json!({
                            "method": request.method,
                        }))),
                    )
                }
            }
        }
    }

    /// Handle the `initialize` request.
    fn handle_initialize(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                },
                "serverInfo": {
                    "name": self.name,
                    "version": self.version,
                },
            }),
        )
    }

    /// Handle the `tools/list` request.
    fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let tools_json: Vec<serde_json::Value> = self
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            })
            .collect();

        JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({ "tools": tools_json }),
        )
    }

    /// Handle the `tools/call` request.
    fn handle_tools_call(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params(None),
                )
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params(Some(
                        serde_json::json!({"message": "missing 'name' parameter"}),
                    )),
                )
            }
        };

        // Verify the tool exists.
        let tool_exists = self.tools.iter().any(|t| t.name == tool_name);
        if !tool_exists {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError {
                    code: error_codes::METHOD_NOT_FOUND,
                    message: format!("tool '{}' not found", tool_name),
                    data: None,
                },
            );
        }

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if let Some(handler) = &self.tool_handler {
            match handler(tool_name, arguments) {
                Ok(result) => JsonRpcResponse::success(
                    request.id.clone(),
                    serde_json::json!({
                        "content": [{"type": "text", "text": result.to_string()}],
                        "isError": false,
                    }),
                ),
                Err(e) => JsonRpcResponse::success(
                    request.id.clone(),
                    serde_json::json!({
                        "content": [{"type": "text", "text": e.to_string()}],
                        "isError": true,
                    }),
                ),
            }
        } else {
            JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::internal_error("no tool handler configured"),
            )
        }
    }

    /// Handle the `resources/list` request.
    fn handle_resources_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // ThunderCode doesn't expose resources as an MCP server currently.
        JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({ "resources": [] }),
        )
    }

    /// Run the server on stdio, reading from stdin and writing to stdout.
    ///
    /// This blocks until stdin is closed.
    pub async fn run_stdio(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(request) => self.handle_request(&request),
                Err(_) => {
                    // Try parsing as any message to get an id.
                    let id = serde_json::from_str::<serde_json::Value>(&line)
                        .ok()
                        .and_then(|v| v.get("id").cloned())
                        .and_then(|id| serde_json::from_value(id).ok())
                        .unwrap_or(jsonrpc::JsonRpcId::Number(0));

                    JsonRpcResponse::error(id, JsonRpcError::parse_error(None))
                }
            };

            let mut response_line =
                serde_json::to_string(&response).context("failed to serialize response")?;
            response_line.push('\n');

            let mut out = stdout.lock().await;
            out.write_all(response_line.as_bytes()).await?;
            out.flush().await?;
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::jsonrpc::JsonRpcId;

    fn make_test_server() -> McpServer {
        let tools = vec![
            McpToolDescription {
                name: "echo".to_string(),
                description: "Echo the input".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    }
                }),
            },
            McpToolDescription {
                name: "add".to_string(),
                description: "Add two numbers".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "a": {"type": "number"},
                        "b": {"type": "number"}
                    }
                }),
            },
        ];

        McpServer::new("test-server", "1.0.0", tools).with_tool_handler(|name, args| {
            match name {
                "echo" => {
                    let msg = args
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(empty)");
                    Ok(serde_json::json!(msg))
                }
                "add" => {
                    let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    Ok(serde_json::json!(a + b))
                }
                _ => anyhow::bail!("unknown tool: {}", name),
            }
        })
    }

    #[test]
    fn test_handle_initialize() {
        let server = make_test_server();
        let req = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1.0"}
            })),
            JsonRpcId::Number(1),
        );

        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.into_result().unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "test-server");
        assert_eq!(result["serverInfo"]["version"], "1.0.0");
    }

    #[test]
    fn test_handle_tools_list() {
        let server = make_test_server();
        let req = JsonRpcRequest::new("tools/list", None, JsonRpcId::Number(2));

        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.into_result().unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "echo");
        assert_eq!(tools[1]["name"], "add");
    }

    #[test]
    fn test_handle_tools_call_success() {
        let server = make_test_server();
        let req = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({
                "name": "echo",
                "arguments": {"message": "hello world"}
            })),
            JsonRpcId::Number(3),
        );

        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.into_result().unwrap();
        assert_eq!(result["isError"], false);
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert!(content[0]["text"].as_str().unwrap().contains("hello world"));
    }

    #[test]
    fn test_handle_tools_call_unknown_tool() {
        let server = make_test_server();
        let req = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({
                "name": "nonexistent",
                "arguments": {}
            })),
            JsonRpcId::Number(4),
        );

        let resp = server.handle_request(&req);
        assert!(resp.is_error());
    }

    #[test]
    fn test_handle_tools_call_no_params() {
        let server = make_test_server();
        let req = JsonRpcRequest::new("tools/call", None, JsonRpcId::Number(5));

        let resp = server.handle_request(&req);
        assert!(resp.is_error());
        let err = resp.into_result().unwrap_err();
        assert_eq!(err.code, error_codes::INVALID_PARAMS);
    }

    #[test]
    fn test_handle_unknown_method() {
        let server = make_test_server();
        let req = JsonRpcRequest::new("unknown/method", None, JsonRpcId::Number(6));

        let resp = server.handle_request(&req);
        assert!(resp.is_error());
        let err = resp.into_result().unwrap_err();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn test_handle_notification() {
        let server = make_test_server();
        let req = JsonRpcRequest::new(
            "notifications/initialized",
            None,
            JsonRpcId::Number(7),
        );

        let resp = server.handle_request(&req);
        // Notifications should succeed silently.
        assert!(!resp.is_error());
    }

    #[test]
    fn test_handle_ping() {
        let server = make_test_server();
        let req = JsonRpcRequest::new("ping", None, JsonRpcId::Number(8));

        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
    }

    #[test]
    fn test_handle_resources_list() {
        let server = make_test_server();
        let req = JsonRpcRequest::new("resources/list", None, JsonRpcId::Number(9));

        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.into_result().unwrap();
        let resources = result["resources"].as_array().unwrap();
        assert!(resources.is_empty());
    }

    #[test]
    fn test_server_without_handler() {
        let server = McpServer::new("bare", "0.0.1", vec![
            McpToolDescription {
                name: "test".to_string(),
                description: "test".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            },
        ]);

        let req = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({"name": "test", "arguments": {}})),
            JsonRpcId::Number(10),
        );

        let resp = server.handle_request(&req);
        assert!(resp.is_error());
        let err = resp.into_result().unwrap_err();
        assert_eq!(err.code, error_codes::INTERNAL_ERROR);
    }
}
