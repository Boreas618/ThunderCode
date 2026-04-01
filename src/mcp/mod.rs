//! ThunderCode Model Context Protocol (MCP) implementation.
//!
//! This crate provides:
//!
//! - **JSON-RPC 2.0** message types (`jsonrpc`) for the MCP wire protocol.
//! - **MCP types** (`types`) for server configs, connection states, tools,
//!   and resources.
//! - **Transports** (`transport`) -- stdio, HTTP, and WebSocket.
//! - **Client** (`client`) -- connect to external MCP servers, list tools,
//!   call tools, and read resources.
//! - **Server** (`server`) -- expose ThunderCode tools as an MCP server.
//! - **Discovery** (`discovery`) -- find MCP server configs in `.mcp.json`
//!   files and settings.

pub mod jsonrpc;
pub mod types;
pub mod transport;
pub mod client;
pub mod server;
pub mod discovery;

// Re-export the most commonly used types at the crate root.
pub use client::McpClient;
pub use discovery::{discover_mcp_servers, load_mcp_json};
pub use jsonrpc::{JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse};
pub use server::McpServer;
pub use transport::McpTransport;
pub use types::{
    ConnectedMcpServer, McpConnectionState, McpResource, McpServerConfig, McpToolDescription,
    McpTransportType,
};
