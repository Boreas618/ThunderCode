//! Plugin marketplace client.
//!
//! Provides a client for listing and installing plugins from a remote
//! marketplace. Currently a stub implementation that returns empty results --
//! real marketplace integration will be added when the backend is available.

use anyhow::Result;
use crate::types::plugin::PluginManifest;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// MarketplaceClient
// ---------------------------------------------------------------------------

/// Client for interacting with the plugin marketplace.
///
/// The marketplace allows users to discover and install community plugins.
/// This is currently a stub; the real implementation will make HTTP requests
/// to the marketplace API.
#[derive(Debug, Clone, Default)]
pub struct MarketplaceClient;

impl MarketplaceClient {
    /// Create a new marketplace client.
    pub fn new() -> Self {
        Self
    }

    /// List available plugins from the marketplace.
    ///
    /// Returns manifests for all plugins published to the marketplace.
    pub async fn list_plugins(&self) -> Result<Vec<PluginManifest>> {
        // Stub: marketplace integration not yet implemented.
        Ok(Vec::new())
    }

    /// Install a plugin by name from the marketplace.
    ///
    /// Downloads the plugin into the local plugins directory and returns the
    /// path to the installed plugin directory.
    pub async fn install_plugin(&self, name: &str) -> Result<PathBuf> {
        // Stub: marketplace integration not yet implemented.
        anyhow::bail!(
            "marketplace installation not yet implemented (requested: {})",
            name
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_plugins_returns_empty() {
        let client = MarketplaceClient::new();
        let plugins = client.list_plugins().await.unwrap();
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_install_plugin_not_implemented() {
        let client = MarketplaceClient::new();
        let result = client.install_plugin("some-plugin").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not yet implemented"));
        assert!(msg.contains("some-plugin"));
    }
}
