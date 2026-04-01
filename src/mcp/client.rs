//! MCP client: manages connection to a single MCP server.
//!
//! Handles the MCP lifecycle: initialize, list tools/resources, call tools,
//! read resources, and disconnect.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;

use crate::mcp::jsonrpc::{self, JsonRpcRequest};
use crate::mcp::transport::{HttpTransport, McpTransport, StdioTransport, WebSocketTransport};
use crate::mcp::types::{
    ConnectedMcpServer, FailedMcpServer, McpConnectionState, McpResource, McpServerConfig,
    McpToolDescription, McpTransportType, ServerCapabilities, ServerInfo,
};

// ============================================================================
// Constants
// ============================================================================

/// Client info sent during the MCP `initialize` handshake.
const CLIENT_NAME: &str = "thundercode";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// MCP protocol version we support.
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// ============================================================================
// McpClient
// ============================================================================

/// Client for a single MCP server.
///
/// Manages the transport, connection state, and MCP protocol interactions.
pub struct McpClient {
    config: McpServerConfig,
    state: McpConnectionState,
    transport: Option<Arc<dyn McpTransport>>,
    /// Mutex to serialize protocol operations.
    _lock: Mutex<()>,
}

impl McpClient {
    /// Attempt to connect to an MCP server with the given configuration.
    ///
    /// This spawns the transport, performs the MCP `initialize` handshake,
    /// and transitions to a connected state (or fails).
    pub async fn connect(config: McpServerConfig) -> Result<Self> {
        let transport = match Self::create_transport(&config).await {
            Ok(t) => t,
            Err(e) => {
                return Ok(McpClient {
                    config: config.clone(),
                    state: McpConnectionState::Failed(FailedMcpServer {
                        name: config.name.clone(),
                        error: e.to_string(),
                    }),
                    transport: None,
                    _lock: Mutex::new(()),
                });
            }
        };
        let transport: Arc<dyn McpTransport> = Arc::from(transport);

        // Perform the MCP initialize handshake.
        match Self::do_initialize(&transport, &config.name).await {
            Ok((capabilities, server_info, instructions)) => {
                // Fetch tools and resources.
                let tools = Self::fetch_tools(&transport).await.unwrap_or_default();
                let resources = Self::fetch_resources(&transport).await.unwrap_or_default();

                // Send initialized notification (no response expected, but we
                // send it as a fire-and-forget request to keep the transport
                // simple -- the server may ignore the id).
                let notif_req = JsonRpcRequest::new(
                    "notifications/initialized",
                    None,
                    jsonrpc::next_request_id(),
                );
                // Best-effort; don't fail if the server doesn't respond.
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    transport.send(&notif_req),
                )
                .await;

                let connected = ConnectedMcpServer {
                    name: config.name.clone(),
                    tools,
                    resources,
                    capabilities,
                    server_info,
                    instructions,
                };

                Ok(McpClient {
                    config,
                    state: McpConnectionState::Connected(connected),
                    transport: Some(transport),
                    _lock: Mutex::new(()),
                })
            }
            Err(e) => {
                // Clean up transport on failure.
                let _ = transport.close().await;
                Ok(McpClient {
                    config: config.clone(),
                    state: McpConnectionState::Failed(FailedMcpServer {
                        name: config.name.clone(),
                        error: e.to_string(),
                    }),
                    transport: None,
                    _lock: Mutex::new(()),
                })
            }
        }
    }

    /// Create the appropriate transport for the given config.
    async fn create_transport(config: &McpServerConfig) -> Result<Box<dyn McpTransport>> {
        match config.transport {
            McpTransportType::Stdio => {
                let command = config
                    .command
                    .as_deref()
                    .context("stdio transport requires a command")?;
                let env = config.env.clone().unwrap_or_default();
                let transport =
                    StdioTransport::spawn(command, &config.args, &env).await?;
                Ok(Box::new(transport))
            }
            McpTransportType::Sse | McpTransportType::Http | McpTransportType::ApiProxy => {
                let url = config
                    .url
                    .as_deref()
                    .context("HTTP/SSE transport requires a URL")?;
                let headers = config.headers.clone().unwrap_or_default();
                let transport = HttpTransport::new(url, headers);
                Ok(Box::new(transport))
            }
            McpTransportType::Ws | McpTransportType::WsIde => {
                let url = config
                    .url
                    .as_deref()
                    .context("WebSocket transport requires a URL")?;
                let transport = WebSocketTransport::connect(url).await?;
                Ok(Box::new(transport))
            }
            McpTransportType::SseIde | McpTransportType::Sdk => {
                anyhow::bail!(
                    "transport type {:?} is not supported in this context",
                    config.transport
                )
            }
        }
    }

    /// Perform the MCP `initialize` handshake.
    async fn do_initialize(
        transport: &Arc<dyn McpTransport>,
        server_name: &str,
    ) -> Result<(ServerCapabilities, Option<ServerInfo>, Option<String>)> {
        let params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": CLIENT_NAME,
                "version": CLIENT_VERSION,
            }
        });

        let request = JsonRpcRequest::new("initialize", Some(params), jsonrpc::next_request_id());

        let response = transport
            .send(&request)
            .await
            .with_context(|| format!("MCP initialize failed for server '{}'", server_name))?;

        let result = response
            .into_result()
            .map_err(|e| anyhow::anyhow!("MCP initialize error: {}", e))?;

        let capabilities: ServerCapabilities =
            serde_json::from_value(result.get("capabilities").cloned().unwrap_or_default())
                .unwrap_or_default();

        let server_info: Option<ServerInfo> = result
            .get("serverInfo")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());

        let instructions: Option<String> = result
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok((capabilities, server_info, instructions))
    }

    /// Fetch the list of tools from a connected server.
    async fn fetch_tools(transport: &Arc<dyn McpTransport>) -> Result<Vec<McpToolDescription>> {
        let request = JsonRpcRequest::new("tools/list", None, jsonrpc::next_request_id());
        let response = transport.send(&request).await?;
        let result = response.into_result().map_err(|e| anyhow::anyhow!("{}", e))?;

        let tools: Vec<McpToolDescription> = result
            .get("tools")
            .cloned()
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        Ok(tools)
    }

    /// Fetch the list of resources from a connected server.
    async fn fetch_resources(transport: &Arc<dyn McpTransport>) -> Result<Vec<McpResource>> {
        let request =
            JsonRpcRequest::new("resources/list", None, jsonrpc::next_request_id());
        let response = transport.send(&request).await?;
        let result = response.into_result().map_err(|e| anyhow::anyhow!("{}", e))?;

        let resources: Vec<McpResource> = result
            .get("resources")
            .cloned()
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        Ok(resources)
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Get the current connection state.
    pub fn state(&self) -> &McpConnectionState {
        &self.state
    }

    /// Get the server configuration.
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Returns `true` if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.state.is_connected()
    }

    /// Disconnect from the server and release resources.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(transport) = self.transport.take() {
            transport.close().await?;
        }
        self.state = McpConnectionState::Disabled;
        Ok(())
    }

    /// List tools available on the connected server.
    ///
    /// Returns the cached tools from initialization. For a fresh listing,
    /// call `refresh_tools`.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDescription>> {
        match &self.state {
            McpConnectionState::Connected(server) => Ok(server.tools.clone()),
            _ => anyhow::bail!("server '{}' is not connected", self.config.name),
        }
    }

    /// Refresh the tool list from the server.
    pub async fn refresh_tools(&mut self) -> Result<Vec<McpToolDescription>> {
        let transport = self
            .transport
            .as_ref()
            .context("no active transport")?;

        let tools = Self::fetch_tools(transport).await?;

        if let McpConnectionState::Connected(ref mut server) = self.state {
            server.tools = tools.clone();
        }

        Ok(tools)
    }

    /// Call a tool on the connected MCP server.
    pub async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let transport = self
            .transport
            .as_ref()
            .context("no active transport")?;

        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });

        let request =
            JsonRpcRequest::new("tools/call", Some(params), jsonrpc::next_request_id());

        let response = transport
            .send(&request)
            .await
            .with_context(|| format!("tool call '{}' failed", name))?;

        let result = response
            .into_result()
            .map_err(|e| anyhow::anyhow!("MCP tool call error: {}", e))?;

        Ok(result)
    }

    /// List resources available on the connected server.
    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        match &self.state {
            McpConnectionState::Connected(server) => Ok(server.resources.clone()),
            _ => anyhow::bail!("server '{}' is not connected", self.config.name),
        }
    }

    /// Refresh the resource list from the server.
    pub async fn refresh_resources(&mut self) -> Result<Vec<McpResource>> {
        let transport = self
            .transport
            .as_ref()
            .context("no active transport")?;

        let resources = Self::fetch_resources(transport).await?;

        if let McpConnectionState::Connected(ref mut server) = self.state {
            server.resources = resources.clone();
        }

        Ok(resources)
    }

    /// Read a resource from the MCP server by URI.
    pub async fn read_resource(&self, uri: &str) -> Result<String> {
        let transport = self
            .transport
            .as_ref()
            .context("no active transport")?;

        let params = serde_json::json!({ "uri": uri });
        let request =
            JsonRpcRequest::new("resources/read", Some(params), jsonrpc::next_request_id());

        let response = transport
            .send(&request)
            .await
            .with_context(|| format!("failed to read resource '{}'", uri))?;

        let result = response
            .into_result()
            .map_err(|e| anyhow::anyhow!("MCP resource read error: {}", e))?;

        // The MCP spec returns content as an array of content items.
        // We extract text from the first text content item.
        if let Some(contents) = result.get("contents").and_then(|v| v.as_array()) {
            for content in contents {
                if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        // Fall back to returning the full result as a JSON string.
        Ok(serde_json::to_string_pretty(&result)?)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stdio_config(name: &str, command: &str) -> McpServerConfig {
        McpServerConfig {
            name: name.to_string(),
            transport: McpTransportType::Stdio,
            command: Some(command.to_string()),
            url: None,
            env: None,
            args: vec![],
            headers: None,
        }
    }

    fn make_http_config(name: &str, url: &str) -> McpServerConfig {
        McpServerConfig {
            name: name.to_string(),
            transport: McpTransportType::Http,
            command: None,
            url: Some(url.to_string()),
            env: None,
            args: vec![],
            headers: None,
        }
    }

    #[test]
    fn test_client_config() {
        let config = make_stdio_config("test", "echo");
        assert_eq!(config.name, "test");
        assert_eq!(config.transport, McpTransportType::Stdio);
    }

    #[tokio::test]
    async fn test_connect_nonexistent_stdio_server() {
        let config = make_stdio_config("test", "/nonexistent/mcp-server-xyz");
        let client = McpClient::connect(config).await.unwrap();
        // Should be in a failed state because the binary doesn't exist.
        assert!(!client.is_connected());
        assert_eq!(client.state().state_type(), "failed");
    }

    #[tokio::test]
    async fn test_list_tools_not_connected() {
        let config = make_stdio_config("test", "/nonexistent/binary");
        let client = McpClient::connect(config).await.unwrap();
        let result = client.list_tools().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_not_connected() {
        let config = make_http_config("test", "http://localhost:1");
        let client = McpClient::connect(config).await.unwrap();
        let result = client.call_tool("test", serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_disconnect() {
        let config = make_stdio_config("test", "/nonexistent/binary");
        let mut client = McpClient::connect(config).await.unwrap();
        let result = client.disconnect().await;
        assert!(result.is_ok());
        assert_eq!(client.state().state_type(), "disabled");
    }

    #[test]
    fn test_constants() {
        assert_eq!(CLIENT_NAME, "thundercode");
        assert_eq!(MCP_PROTOCOL_VERSION, "2024-11-05");
    }
}
