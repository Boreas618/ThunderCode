//! Plugin types -- manifests, loaded plugins, errors.
//!
//! Ported from ref/types/plugin.ts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// PluginManifest
// ============================================================================

/// The manifest for a plugin, read from its plugin.json or manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<PluginAuthor>,
    /// Additional fields from the manifest.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Plugin author information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Metadata for a named command within a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_to_use: Option<String>,
    /// Additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ============================================================================
// PluginRepository
// ============================================================================

/// Repository configuration for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRepository {
    pub url: String,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
}

// ============================================================================
// PluginConfig
// ============================================================================

/// Plugin configuration (repositories).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginConfig {
    pub repositories: HashMap<String, PluginRepository>,
}

// ============================================================================
// LoadedPlugin
// ============================================================================

/// A plugin that has been loaded from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedPlugin {
    pub name: String,
    pub manifest: PluginManifest,
    pub path: String,
    pub source: String,
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_builtin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands_metadata: Option<HashMap<String, CommandMetadata>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_styles_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_styles_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks_config: Option<serde_json::Value>, // HooksSettings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>, // McpServerConfig
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_servers: Option<HashMap<String, serde_json::Value>>, // LspServerConfig
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<HashMap<String, serde_json::Value>>,
}

// ============================================================================
// PluginComponent
// ============================================================================

/// A component type within a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginComponent {
    Commands,
    Agents,
    Skills,
    Hooks,
    OutputStyles,
}

// ============================================================================
// PluginError
// ============================================================================

/// Discriminated union of plugin error types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginError {
    #[serde(rename = "path-not-found")]
    PathNotFound {
        source: String,
        plugin: Option<String>,
        path: String,
        component: PluginComponent,
    },

    #[serde(rename = "git-auth-failed")]
    GitAuthFailed {
        source: String,
        plugin: Option<String>,
        git_url: String,
        auth_type: GitAuthType,
    },

    #[serde(rename = "git-timeout")]
    GitTimeout {
        source: String,
        plugin: Option<String>,
        git_url: String,
        operation: GitOperation,
    },

    #[serde(rename = "network-error")]
    NetworkError {
        source: String,
        plugin: Option<String>,
        url: String,
        details: Option<String>,
    },

    #[serde(rename = "manifest-parse-error")]
    ManifestParseError {
        source: String,
        plugin: Option<String>,
        manifest_path: String,
        parse_error: String,
    },

    #[serde(rename = "manifest-validation-error")]
    ManifestValidationError {
        source: String,
        plugin: Option<String>,
        manifest_path: String,
        validation_errors: Vec<String>,
    },

    #[serde(rename = "plugin-not-found")]
    PluginNotFound {
        source: String,
        plugin_id: String,
        marketplace: String,
    },

    #[serde(rename = "marketplace-not-found")]
    MarketplaceNotFound {
        source: String,
        marketplace: String,
        available_marketplaces: Vec<String>,
    },

    #[serde(rename = "marketplace-load-failed")]
    MarketplaceLoadFailed {
        source: String,
        marketplace: String,
        reason: String,
    },

    #[serde(rename = "mcp-config-invalid")]
    McpConfigInvalid {
        source: String,
        plugin: String,
        server_name: String,
        validation_error: String,
    },

    #[serde(rename = "mcp-server-suppressed-duplicate")]
    McpServerSuppressedDuplicate {
        source: String,
        plugin: String,
        server_name: String,
        duplicate_of: String,
    },

    #[serde(rename = "lsp-config-invalid")]
    LspConfigInvalid {
        source: String,
        plugin: String,
        server_name: String,
        validation_error: String,
    },

    #[serde(rename = "hook-load-failed")]
    HookLoadFailed {
        source: String,
        plugin: String,
        hook_path: String,
        reason: String,
    },

    #[serde(rename = "component-load-failed")]
    ComponentLoadFailed {
        source: String,
        plugin: String,
        component: PluginComponent,
        path: String,
        reason: String,
    },

    #[serde(rename = "mcpb-download-failed")]
    McpbDownloadFailed {
        source: String,
        plugin: String,
        url: String,
        reason: String,
    },

    #[serde(rename = "mcpb-extract-failed")]
    McpbExtractFailed {
        source: String,
        plugin: String,
        mcpb_path: String,
        reason: String,
    },

    #[serde(rename = "mcpb-invalid-manifest")]
    McpbInvalidManifest {
        source: String,
        plugin: String,
        mcpb_path: String,
        validation_error: String,
    },

    #[serde(rename = "lsp-server-start-failed")]
    LspServerStartFailed {
        source: String,
        plugin: String,
        server_name: String,
        reason: String,
    },

    #[serde(rename = "lsp-server-crashed")]
    LspServerCrashed {
        source: String,
        plugin: String,
        server_name: String,
        exit_code: Option<i32>,
        signal: Option<String>,
    },

    #[serde(rename = "lsp-request-timeout")]
    LspRequestTimeout {
        source: String,
        plugin: String,
        server_name: String,
        method: String,
        timeout_ms: u64,
    },

    #[serde(rename = "lsp-request-failed")]
    LspRequestFailed {
        source: String,
        plugin: String,
        server_name: String,
        method: String,
        error: String,
    },

    #[serde(rename = "marketplace-blocked-by-policy")]
    MarketplaceBlockedByPolicy {
        source: String,
        plugin: Option<String>,
        marketplace: String,
        blocked_by_blocklist: Option<bool>,
        allowed_sources: Vec<String>,
    },

    #[serde(rename = "dependency-unsatisfied")]
    DependencyUnsatisfied {
        source: String,
        plugin: String,
        dependency: String,
        reason: DependencyUnsatisfiedReason,
    },

    #[serde(rename = "plugin-cache-miss")]
    PluginCacheMiss {
        source: String,
        plugin: String,
        install_path: String,
    },

    #[serde(rename = "generic-error")]
    GenericError {
        source: String,
        plugin: Option<String>,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitAuthType {
    Ssh,
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitOperation {
    Clone,
    Pull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyUnsatisfiedReason {
    NotEnabled,
    NotFound,
}

impl PluginError {
    /// Get a display message for any plugin error.
    pub fn display_message(&self) -> String {
        match self {
            PluginError::GenericError { error, .. } => error.clone(),
            PluginError::PathNotFound {
                path, component, ..
            } => format!("Path not found: {} ({:?})", path, component),
            PluginError::GitAuthFailed {
                git_url, auth_type, ..
            } => format!("Git authentication failed ({:?}): {}", auth_type, git_url),
            PluginError::GitTimeout {
                git_url, operation, ..
            } => format!("Git {:?} timeout: {}", operation, git_url),
            PluginError::NetworkError { url, details, .. } => {
                if let Some(d) = details {
                    format!("Network error: {} - {}", url, d)
                } else {
                    format!("Network error: {}", url)
                }
            }
            PluginError::ManifestParseError { parse_error, .. } => {
                format!("Manifest parse error: {}", parse_error)
            }
            PluginError::ManifestValidationError {
                validation_errors, ..
            } => format!(
                "Manifest validation failed: {}",
                validation_errors.join(", ")
            ),
            PluginError::PluginNotFound {
                plugin_id,
                marketplace,
                ..
            } => format!(
                "Plugin {} not found in marketplace {}",
                plugin_id, marketplace
            ),
            PluginError::MarketplaceNotFound { marketplace, .. } => {
                format!("Marketplace {} not found", marketplace)
            }
            PluginError::MarketplaceLoadFailed {
                marketplace,
                reason,
                ..
            } => format!("Marketplace {} failed to load: {}", marketplace, reason),
            PluginError::McpConfigInvalid {
                server_name,
                validation_error,
                ..
            } => format!("MCP server {} invalid: {}", server_name, validation_error),
            PluginError::McpServerSuppressedDuplicate {
                server_name,
                duplicate_of,
                ..
            } => format!(
                "MCP server \"{}\" skipped -- same command/URL as {}",
                server_name, duplicate_of
            ),
            PluginError::LspConfigInvalid {
                plugin,
                server_name,
                validation_error,
                ..
            } => format!(
                "Plugin \"{}\" has invalid LSP server config for \"{}\": {}",
                plugin, server_name, validation_error
            ),
            PluginError::HookLoadFailed { reason, .. } => {
                format!("Hook load failed: {}", reason)
            }
            PluginError::ComponentLoadFailed {
                component,
                path,
                reason,
                ..
            } => format!("{:?} load failed from {}: {}", component, path, reason),
            PluginError::McpbDownloadFailed { url, reason, .. } => {
                format!("Failed to download MCPB from {}: {}", url, reason)
            }
            PluginError::McpbExtractFailed {
                mcpb_path, reason, ..
            } => format!("Failed to extract MCPB {}: {}", mcpb_path, reason),
            PluginError::McpbInvalidManifest {
                mcpb_path,
                validation_error,
                ..
            } => format!(
                "MCPB manifest invalid at {}: {}",
                mcpb_path, validation_error
            ),
            PluginError::LspServerStartFailed {
                plugin,
                server_name,
                reason,
                ..
            } => format!(
                "Plugin \"{}\" failed to start LSP server \"{}\": {}",
                plugin, server_name, reason
            ),
            PluginError::LspServerCrashed {
                plugin,
                server_name,
                exit_code,
                signal,
                ..
            } => {
                if let Some(sig) = signal {
                    format!(
                        "Plugin \"{}\" LSP server \"{}\" crashed with signal {}",
                        plugin, server_name, sig
                    )
                } else {
                    format!(
                        "Plugin \"{}\" LSP server \"{}\" crashed with exit code {}",
                        plugin,
                        server_name,
                        exit_code.map(|c| c.to_string()).unwrap_or("unknown".into())
                    )
                }
            }
            PluginError::LspRequestTimeout {
                plugin,
                server_name,
                method,
                timeout_ms,
                ..
            } => format!(
                "Plugin \"{}\" LSP server \"{}\" timed out on {} request after {}ms",
                plugin, server_name, method, timeout_ms
            ),
            PluginError::LspRequestFailed {
                plugin,
                server_name,
                method,
                error,
                ..
            } => format!(
                "Plugin \"{}\" LSP server \"{}\" {} request failed: {}",
                plugin, server_name, method, error
            ),
            PluginError::MarketplaceBlockedByPolicy {
                marketplace,
                blocked_by_blocklist,
                ..
            } => {
                if *blocked_by_blocklist == Some(true) {
                    format!(
                        "Marketplace '{}' is blocked by enterprise policy",
                        marketplace
                    )
                } else {
                    format!(
                        "Marketplace '{}' is not in the allowed marketplace list",
                        marketplace
                    )
                }
            }
            PluginError::DependencyUnsatisfied {
                dependency, reason, ..
            } => {
                let hint = match reason {
                    DependencyUnsatisfiedReason::NotEnabled => {
                        "disabled -- enable it or remove the dependency"
                    }
                    DependencyUnsatisfiedReason::NotFound => {
                        "not found in any configured marketplace"
                    }
                };
                format!("Dependency \"{}\" is {}", dependency, hint)
            }
            PluginError::PluginCacheMiss {
                plugin,
                install_path,
                ..
            } => format!(
                "Plugin \"{}\" not cached at {} -- run /plugins to refresh",
                plugin, install_path
            ),
        }
    }
}

// ============================================================================
// PluginLoadResult
// ============================================================================

/// Result of loading plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLoadResult {
    pub enabled: Vec<LoadedPlugin>,
    pub disabled: Vec<LoadedPlugin>,
    pub errors: Vec<PluginError>,
}

// ============================================================================
// BuiltinPluginDefinition
// ============================================================================

/// Definition for a built-in plugin that ships with the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinPluginDefinition {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Default enabled state (defaults to true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_enabled: Option<bool>,
}
