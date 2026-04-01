//! MCP protocol types: transport variants, server configs, connection states,
//! tool descriptions, and resources.
//!
//! Ported from ref/services/mcp/types.ts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// McpTransportType
// ============================================================================

/// The transport mechanism used to communicate with an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransportType {
    Stdio,
    Sse,
    SseIde,
    Http,
    Ws,
    WsIde,
    Sdk,
    #[serde(rename = "api-proxy")]
    ApiProxy,
}

impl std::fmt::Display for McpTransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpTransportType::Stdio => write!(f, "stdio"),
            McpTransportType::Sse => write!(f, "sse"),
            McpTransportType::SseIde => write!(f, "sse-ide"),
            McpTransportType::Http => write!(f, "http"),
            McpTransportType::Ws => write!(f, "ws"),
            McpTransportType::WsIde => write!(f, "ws-ide"),
            McpTransportType::Sdk => write!(f, "sdk"),
            McpTransportType::ApiProxy => write!(f, "api-proxy"),
        }
    }
}

// ============================================================================
// ConfigScope
// ============================================================================

/// Where a server configuration was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigScope {
    Local,
    User,
    Project,
    Dynamic,
    Enterprise,
    #[serde(rename = "api-auth")]
    ApiAuth,
    Managed,
}

// ============================================================================
// McpServerConfig
// ============================================================================

/// Configuration for connecting to an MCP server.
///
/// This is a unified representation -- the TypeScript reference uses a
/// discriminated union of per-transport config types. Here we keep one
/// struct with optional fields; the transport type determines which fields
/// are meaningful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Human-readable server name (key from the mcpServers map).
    pub name: String,
    /// Transport mechanism.
    pub transport: McpTransportType,
    /// Command to spawn (stdio transport). First element is the binary,
    /// rest are arguments to prepend before `args`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// URL for remote transports (sse, http, ws, api-proxy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Environment variables to set for a spawned process (stdio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Additional arguments for stdio transport.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// HTTP headers for remote transports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// Server config tagged with its originating scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedMcpServerConfig {
    #[serde(flatten)]
    pub config: McpServerConfig,
    pub scope: ConfigScope,
}

// ============================================================================
// Connection states
// ============================================================================

/// The state of a connection to an MCP server.
#[derive(Debug, Clone)]
pub enum McpConnectionState {
    /// Connecting / waiting for the server to respond.
    Pending,
    /// Successfully connected.
    Connected(ConnectedMcpServer),
    /// Connection or initialization failed.
    Failed(FailedMcpServer),
    /// Server requires authentication (e.g. OAuth).
    NeedsAuth(NeedsAuthMcpServer),
    /// Server is explicitly disabled by the user.
    Disabled,
}

impl McpConnectionState {
    /// Returns the state type as a string label.
    pub fn state_type(&self) -> &'static str {
        match self {
            McpConnectionState::Pending => "pending",
            McpConnectionState::Connected(_) => "connected",
            McpConnectionState::Failed(_) => "failed",
            McpConnectionState::NeedsAuth(_) => "needs-auth",
            McpConnectionState::Disabled => "disabled",
        }
    }

    /// Returns `true` if this is a connected state.
    pub fn is_connected(&self) -> bool {
        matches!(self, McpConnectionState::Connected(_))
    }
}

/// A successfully connected MCP server.
#[derive(Debug, Clone)]
pub struct ConnectedMcpServer {
    pub name: String,
    pub tools: Vec<McpToolDescription>,
    pub resources: Vec<McpResource>,
    /// Server capabilities declared during initialization.
    pub capabilities: ServerCapabilities,
    /// Server name and version as reported by the server.
    pub server_info: Option<ServerInfo>,
    /// Server-provided instructions for the LLM.
    pub instructions: Option<String>,
}

/// Server capabilities declared in the MCP `initialize` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
}

/// Server identity reported in the `initialize` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// A connection attempt that failed.
#[derive(Debug, Clone)]
pub struct FailedMcpServer {
    pub name: String,
    pub error: String,
}

/// A server that requires authentication before connecting.
#[derive(Debug, Clone)]
pub struct NeedsAuthMcpServer {
    pub name: String,
}

// ============================================================================
// McpToolDescription
// ============================================================================

/// Description of a tool exposed by an MCP server.
///
/// Matches the MCP `Tool` object in the `tools/list` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDescription {
    /// The tool's name (unique within the server).
    pub name: String,
    /// Human-readable description of what the tool does.
    #[serde(default)]
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    #[serde(default = "default_input_schema", rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({"type": "object"})
}

// ============================================================================
// McpResource
// ============================================================================

/// A resource exposed by an MCP server.
///
/// Matches the MCP `Resource` object in the `resources/list` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// URI identifying the resource.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content.
    #[serde(skip_serializing_if = "Option::is_none", rename = "mimeType")]
    pub mime_type: Option<String>,
}

// ============================================================================
// MCP tool call result content
// ============================================================================

/// A single content item in an MCP tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: McpResourceContent },
}

/// Embedded resource content in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mimeType")]
    pub mime_type: Option<String>,
}

// ============================================================================
// Normalization helpers
// ============================================================================

/// Normalize a server name to be compatible with the MCP tool name pattern
/// `^[a-zA-Z0-9_-]{1,64}$`.
///
/// Replaces invalid characters with underscores. For primary.ai servers
/// (names starting with "primary.ai "), also collapses consecutive underscores
/// and strips leading/trailing underscores.
pub fn normalize_name_for_mcp(name: &str) -> String {
    let mut normalized: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();

    if name.starts_with("primary.ai ") {
        // Collapse runs of underscores
        while normalized.contains("__") {
            normalized = normalized.replace("__", "_");
        }
        normalized = normalized.trim_matches('_').to_string();
    }

    normalized
}

/// Build the full MCP tool name: `mcp__<server>__<tool>`.
pub fn build_mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "mcp__{}__{}",
        normalize_name_for_mcp(server_name),
        normalize_name_for_mcp(tool_name),
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_serde() {
        let t = McpTransportType::Stdio;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"stdio\"");

        let parsed: McpTransportType = serde_json::from_str("\"sse\"").unwrap();
        assert_eq!(parsed, McpTransportType::Sse);

        let parsed: McpTransportType = serde_json::from_str("\"sse-ide\"").unwrap();
        assert_eq!(parsed, McpTransportType::SseIde);

        let parsed: McpTransportType = serde_json::from_str("\"api-proxy\"").unwrap();
        assert_eq!(parsed, McpTransportType::ApiProxy);
    }

    #[test]
    fn test_transport_type_display() {
        assert_eq!(McpTransportType::Stdio.to_string(), "stdio");
        assert_eq!(McpTransportType::Http.to_string(), "http");
        assert_eq!(McpTransportType::WsIde.to_string(), "ws-ide");
    }

    #[test]
    fn test_config_scope_serde() {
        let scope = ConfigScope::Project;
        let json = serde_json::to_string(&scope).unwrap();
        assert_eq!(json, "\"project\"");

        let parsed: ConfigScope = serde_json::from_str("\"api-auth\"").unwrap();
        assert_eq!(parsed, ConfigScope::ApiAuth);
    }

    #[test]
    fn test_tool_description_serde() {
        let tool = McpToolDescription {
            name: "read_file".to_string(),
            description: "Read a file from disk".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolDescription = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "read_file");
        assert_eq!(parsed.input_schema["properties"]["path"]["type"], "string");
    }

    #[test]
    fn test_resource_serde() {
        let res = McpResource {
            uri: "file:///tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            description: Some("A test file".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("mimeType"));
        let parsed: McpResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "file:///tmp/test.txt");
    }

    #[test]
    fn test_normalize_name_basic() {
        assert_eq!(normalize_name_for_mcp("my-server"), "my-server");
        assert_eq!(normalize_name_for_mcp("my server"), "my_server");
        assert_eq!(normalize_name_for_mcp("my.server.v2"), "my_server_v2");
    }

    #[test]
    fn test_normalize_name_api_auth() {
        // primary.ai servers get extra normalization
        assert_eq!(
            normalize_name_for_mcp("primary.ai my-server"),
            "api_auth_my-server"
        );
        // Should not have leading/trailing underscores or double underscores
        let result = normalize_name_for_mcp("primary.ai  test");
        assert!(!result.starts_with('_'));
        assert!(!result.ends_with('_'));
        assert!(!result.contains("__"));
    }

    #[test]
    fn test_build_mcp_tool_name() {
        assert_eq!(
            build_mcp_tool_name("my-server", "read_file"),
            "mcp__my-server__read_file"
        );
        assert_eq!(
            build_mcp_tool_name("my server", "do.thing"),
            "mcp__my_server__do_thing"
        );
    }

    #[test]
    fn test_connection_state_type() {
        let pending = McpConnectionState::Pending;
        assert_eq!(pending.state_type(), "pending");
        assert!(!pending.is_connected());

        let connected = McpConnectionState::Connected(ConnectedMcpServer {
            name: "test".to_string(),
            tools: vec![],
            resources: vec![],
            capabilities: ServerCapabilities::default(),
            server_info: None,
            instructions: None,
        });
        assert_eq!(connected.state_type(), "connected");
        assert!(connected.is_connected());

        let failed = McpConnectionState::Failed(FailedMcpServer {
            name: "test".to_string(),
            error: "timeout".to_string(),
        });
        assert_eq!(failed.state_type(), "failed");
    }

    #[test]
    fn test_tool_result_content_text() {
        let content = McpToolResultContent::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn test_server_config_serde() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            transport: McpTransportType::Stdio,
            command: Some("node".to_string()),
            url: None,
            env: Some(HashMap::from([("KEY".to_string(), "val".to_string())])),
            args: vec!["server.js".to_string()],
            headers: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: McpServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test-server");
        assert_eq!(parsed.transport, McpTransportType::Stdio);
        assert_eq!(parsed.command.unwrap(), "node");
        assert_eq!(parsed.args, vec!["server.js"]);
    }
}
