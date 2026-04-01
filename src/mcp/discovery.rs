//! MCP server discovery: find and load MCP server configurations
//! from settings, project files, and global config.
//!
//! Ported from ref/services/mcp/config.ts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::mcp::types::{McpServerConfig, McpTransportType};

// ============================================================================
// McpJsonConfig -- the `.mcp.json` file format
// ============================================================================

/// The on-disk format of `.mcp.json` files.
///
/// ```json
/// {
///   "mcpServers": {
///     "my-server": {
///       "command": "node",
///       "args": ["server.js"],
///       "env": {"API_KEY": "secret"}
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpJsonConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpJsonServerEntry>,
}

/// A single server entry in the `.mcp.json` file.
///
/// The format supports either stdio (command-based) or remote (url-based)
/// server types. The `type` field discriminates; defaults to stdio when absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpJsonServerEntry {
    /// Transport type. Defaults to "stdio" when absent.
    #[serde(default, rename = "type")]
    pub transport_type: Option<String>,
    /// Command to spawn (stdio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Arguments for the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the spawned process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// URL for remote transports (sse, http, ws).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// HTTP headers for remote transports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

impl McpJsonServerEntry {
    /// Convert this entry into an `McpServerConfig` with the given name.
    pub fn into_config(self, name: String) -> McpServerConfig {
        let transport = match self.transport_type.as_deref() {
            Some("sse") => McpTransportType::Sse,
            Some("sse-ide") => McpTransportType::SseIde,
            Some("http") => McpTransportType::Http,
            Some("ws") => McpTransportType::Ws,
            Some("ws-ide") => McpTransportType::WsIde,
            Some("sdk") => McpTransportType::Sdk,
            Some("api-proxy") => McpTransportType::ApiProxy,
            // Default to stdio when type is absent or "stdio".
            _ => McpTransportType::Stdio,
        };

        McpServerConfig {
            name,
            transport,
            command: self.command,
            url: self.url,
            env: self.env,
            args: self.args,
            headers: self.headers,
        }
    }
}

// ============================================================================
// Loading and discovery functions
// ============================================================================

/// Load MCP server configurations from a `.mcp.json` file.
///
/// Returns a map of server name to config. Returns an empty map if the
/// file does not exist.
pub fn load_mcp_json(path: &Path) -> Result<HashMap<String, McpServerConfig>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read MCP config: {}", path.display()))?;

    let config: McpJsonConfig = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse MCP config: {}", path.display()))?;

    let servers = config
        .mcp_servers
        .into_iter()
        .map(|(name, entry)| {
            let config = entry.into_config(name.clone());
            (name, config)
        })
        .collect();

    Ok(servers)
}

/// Discover all MCP server configurations from various sources.
///
/// Searches for `.mcp.json` in:
/// 1. The current working directory (project scope)
/// 2. The user's home config directory (`~/.thundercode/.mcp.json`)
///
/// Later entries override earlier ones (project overrides global).
pub fn discover_mcp_servers(cwd: &Path) -> Vec<McpServerConfig> {
    let mut servers: HashMap<String, McpServerConfig> = HashMap::new();

    // 1. Global/user config: ~/.thundercode/.mcp.json
    if let Some(home) = dirs::home_dir() {
        let global_path = home.join(".thundercode").join(".mcp.json");
        if let Ok(global_servers) = load_mcp_json(&global_path) {
            servers.extend(global_servers);
        }
    }

    // 2. Project-local config: <cwd>/.mcp.json
    let project_path = cwd.join(".mcp.json");
    if let Ok(project_servers) = load_mcp_json(&project_path) {
        servers.extend(project_servers);
    }

    servers.into_values().collect()
}

/// Find all `.mcp.json` files in the standard locations.
///
/// Returns paths to existing config files (useful for watching).
pub fn find_mcp_config_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let global_path = home.join(".thundercode").join(".mcp.json");
        if global_path.exists() {
            paths.push(global_path);
        }
    }

    let project_path = cwd.join(".mcp.json");
    if project_path.exists() {
        paths.push(project_path);
    }

    paths
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mcp_json_stdio() {
        let json = r#"{
            "mcpServers": {
                "my-server": {
                    "command": "node",
                    "args": ["server.js", "--port", "3000"],
                    "env": {"API_KEY": "test123"}
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);

        let entry = config.mcp_servers.get("my-server").unwrap();
        assert_eq!(entry.command.as_deref(), Some("node"));
        assert_eq!(entry.args, vec!["server.js", "--port", "3000"]);
        assert_eq!(
            entry.env.as_ref().unwrap().get("API_KEY").unwrap(),
            "test123"
        );

        let config = entry.clone().into_config("my-server".to_string());
        assert_eq!(config.transport, McpTransportType::Stdio);
        assert_eq!(config.command.unwrap(), "node");
    }

    #[test]
    fn test_parse_mcp_json_sse() {
        let json = r#"{
            "mcpServers": {
                "remote": {
                    "type": "sse",
                    "url": "https://example.com/mcp/sse",
                    "headers": {"Authorization": "Bearer token123"}
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        let entry = config.mcp_servers.get("remote").unwrap();
        let server = entry.clone().into_config("remote".to_string());
        assert_eq!(server.transport, McpTransportType::Sse);
        assert_eq!(server.url.unwrap(), "https://example.com/mcp/sse");
        assert_eq!(
            server.headers.unwrap().get("Authorization").unwrap(),
            "Bearer token123"
        );
    }

    #[test]
    fn test_parse_mcp_json_http() {
        let json = r#"{
            "mcpServers": {
                "http-server": {
                    "type": "http",
                    "url": "https://api.example.com/mcp"
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        let entry = config.mcp_servers.get("http-server").unwrap();
        let server = entry.clone().into_config("http-server".to_string());
        assert_eq!(server.transport, McpTransportType::Http);
    }

    #[test]
    fn test_parse_mcp_json_ws() {
        let json = r#"{
            "mcpServers": {
                "ws-server": {
                    "type": "ws",
                    "url": "wss://example.com/mcp"
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        let entry = config.mcp_servers.get("ws-server").unwrap();
        let server = entry.clone().into_config("ws-server".to_string());
        assert_eq!(server.transport, McpTransportType::Ws);
    }

    #[test]
    fn test_parse_multiple_servers() {
        let json = r#"{
            "mcpServers": {
                "local": {
                    "command": "python",
                    "args": ["-m", "mcp_server"]
                },
                "remote": {
                    "type": "http",
                    "url": "https://api.example.com/mcp"
                },
                "ide": {
                    "type": "ws-ide",
                    "url": "ws://localhost:9090"
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 3);
    }

    #[test]
    fn test_empty_mcp_json() {
        let json = r#"{"mcpServers": {}}"#;
        let config: McpJsonConfig = serde_json::from_str(json).unwrap();
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_mcp_json(Path::new("/nonexistent/.mcp.json"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_mcp_json_from_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        std::fs::write(
            &path,
            r#"{
                "mcpServers": {
                    "test": {
                        "command": "echo",
                        "args": ["hello"]
                    }
                }
            }"#,
        )
        .unwrap();

        let servers = load_mcp_json(&path).unwrap();
        assert_eq!(servers.len(), 1);
        let server = servers.get("test").unwrap();
        assert_eq!(server.name, "test");
        assert_eq!(server.transport, McpTransportType::Stdio);
        assert_eq!(server.command.as_deref(), Some("echo"));
    }

    #[test]
    fn test_discover_from_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{
                "mcpServers": {
                    "local-server": {
                        "command": "node",
                        "args": ["index.js"]
                    }
                }
            }"#,
        )
        .unwrap();

        let servers = discover_mcp_servers(dir.path());
        assert!(servers.iter().any(|s| s.name == "local-server"));
    }

    #[test]
    fn test_entry_default_transport_is_stdio() {
        let entry = McpJsonServerEntry {
            transport_type: None,
            command: Some("test".to_string()),
            args: vec![],
            env: None,
            url: None,
            headers: None,
        };
        let config = entry.into_config("test".to_string());
        assert_eq!(config.transport, McpTransportType::Stdio);
    }

    #[test]
    fn test_mcp_json_config_roundtrip() {
        let mut servers = HashMap::new();
        servers.insert(
            "my-server".to_string(),
            McpJsonServerEntry {
                transport_type: None,
                command: Some("python".to_string()),
                args: vec!["-m".to_string(), "server".to_string()],
                env: Some(HashMap::from([("KEY".to_string(), "val".to_string())])),
                url: None,
                headers: None,
            },
        );

        let config = McpJsonConfig {
            mcp_servers: servers,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: McpJsonConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mcp_servers.len(), 1);
    }
}
