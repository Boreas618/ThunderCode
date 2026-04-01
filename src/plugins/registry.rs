//! Plugin registry.
//!
//! Manages both loaded (filesystem / marketplace) plugins and built-in plugins
//! that ship with the CLI. Provides enable/disable toggling and lookup.
//!
//! Ported from the plugin state management in `ref/plugins/builtinPlugins.ts`.

use anyhow::Result;
use crate::types::plugin::{BuiltinPluginDefinition, LoadedPlugin, PluginManifest};
use tracing::warn;

/// The sentinel marketplace name used for built-in plugins.
pub const BUILTIN_MARKETPLACE_NAME: &str = "builtin";

// ---------------------------------------------------------------------------
// PluginRegistry
// ---------------------------------------------------------------------------

/// Central registry for all plugins (loaded and built-in).
#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
    builtin: Vec<BuiltinPluginDefinition>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a built-in plugin definition.
    ///
    /// The definition is stored separately; a corresponding `LoadedPlugin` is
    /// added to the main list so it appears alongside filesystem plugins.
    pub fn register_builtin(&mut self, plugin: BuiltinPluginDefinition) {
        let plugin_id = format!("{}@{}", plugin.name, BUILTIN_MARKETPLACE_NAME);
        let is_enabled = plugin.default_enabled.unwrap_or(true);

        let loaded = LoadedPlugin {
            name: plugin.name.clone(),
            manifest: PluginManifest {
                name: plugin.name.clone(),
                description: Some(plugin.description.clone()),
                version: plugin.version.clone(),
                author: None,
                extra: Default::default(),
            },
            path: BUILTIN_MARKETPLACE_NAME.to_string(),
            source: plugin_id.clone(),
            repository: plugin_id,
            enabled: Some(is_enabled),
            is_builtin: Some(true),
            sha: None,
            commands_path: None,
            commands_paths: None,
            commands_metadata: None,
            agents_path: None,
            agents_paths: None,
            skills_path: None,
            skills_paths: None,
            output_styles_path: None,
            output_styles_paths: None,
            hooks_config: None,
            mcp_servers: None,
            lsp_servers: None,
            settings: None,
        };

        self.builtin.push(plugin);
        self.plugins.push(loaded);
    }

    /// Load a plugin from its manifest and add it to the registry.
    ///
    /// The plugin is enabled by default.
    pub fn load_plugin(&mut self, manifest: PluginManifest) -> Result<()> {
        let name = manifest.name.clone();

        // Check for duplicates.
        if self.plugins.iter().any(|p| p.name == name) {
            warn!("plugin '{}' is already registered, skipping", name);
            return Ok(());
        }

        let source = format!("{}@loaded", name);
        let loaded = LoadedPlugin {
            name: manifest.name.clone(),
            manifest,
            path: String::new(),
            source: source.clone(),
            repository: source,
            enabled: Some(true),
            is_builtin: Some(false),
            sha: None,
            commands_path: None,
            commands_paths: None,
            commands_metadata: None,
            agents_path: None,
            agents_paths: None,
            skills_path: None,
            skills_paths: None,
            output_styles_path: None,
            output_styles_paths: None,
            hooks_config: None,
            mcp_servers: None,
            lsp_servers: None,
            settings: None,
        };

        self.plugins.push(loaded);
        Ok(())
    }

    /// Return references to all currently enabled plugins.
    pub fn get_enabled_plugins(&self) -> Vec<&LoadedPlugin> {
        self.plugins
            .iter()
            .filter(|p| p.enabled.unwrap_or(true))
            .collect()
    }

    /// Check whether a specific plugin is enabled.
    pub fn is_enabled(&self, plugin_name: &str) -> bool {
        self.plugins
            .iter()
            .find(|p| p.name == plugin_name)
            .map(|p| p.enabled.unwrap_or(true))
            .unwrap_or(false)
    }

    /// Enable a plugin by name.
    pub fn enable(&mut self, plugin_name: &str) {
        if let Some(p) = self.plugins.iter_mut().find(|p| p.name == plugin_name) {
            p.enabled = Some(true);
        }
    }

    /// Disable a plugin by name.
    pub fn disable(&mut self, plugin_name: &str) {
        if let Some(p) = self.plugins.iter_mut().find(|p| p.name == plugin_name) {
            p.enabled = Some(false);
        }
    }

    /// Get all loaded plugins (enabled and disabled).
    pub fn all_plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    /// Get all registered built-in plugin definitions.
    pub fn builtin_definitions(&self) -> &[BuiltinPluginDefinition] {
        &self.builtin
    }

    /// Check whether a plugin ID represents a built-in plugin (`name@builtin`).
    pub fn is_builtin_plugin_id(plugin_id: &str) -> bool {
        plugin_id.ends_with(&format!("@{}", BUILTIN_MARKETPLACE_NAME))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_builtin(name: &str) -> BuiltinPluginDefinition {
        BuiltinPluginDefinition {
            name: name.to_string(),
            description: format!("{} description", name),
            version: Some("1.0.0".to_string()),
            default_enabled: Some(true),
        }
    }

    fn make_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            description: Some(format!("{} description", name)),
            version: Some("0.1.0".to_string()),
            author: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let reg = PluginRegistry::new();
        assert!(reg.all_plugins().is_empty());
        assert!(reg.builtin_definitions().is_empty());
        assert!(reg.get_enabled_plugins().is_empty());
    }

    #[test]
    fn test_register_builtin() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(make_builtin("test-plugin"));

        assert_eq!(reg.all_plugins().len(), 1);
        assert_eq!(reg.builtin_definitions().len(), 1);
        assert_eq!(reg.all_plugins()[0].name, "test-plugin");
        assert_eq!(reg.all_plugins()[0].is_builtin, Some(true));
        assert_eq!(
            reg.all_plugins()[0].source,
            "test-plugin@builtin"
        );
    }

    #[test]
    fn test_register_builtin_default_enabled() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(make_builtin("enabled-plugin"));

        assert!(reg.is_enabled("enabled-plugin"));
        assert_eq!(reg.get_enabled_plugins().len(), 1);
    }

    #[test]
    fn test_register_builtin_default_disabled() {
        let mut reg = PluginRegistry::new();
        let mut def = make_builtin("disabled-plugin");
        def.default_enabled = Some(false);
        reg.register_builtin(def);

        assert!(!reg.is_enabled("disabled-plugin"));
        assert!(reg.get_enabled_plugins().is_empty());
    }

    #[test]
    fn test_load_plugin() {
        let mut reg = PluginRegistry::new();
        reg.load_plugin(make_manifest("my-plugin")).unwrap();

        assert_eq!(reg.all_plugins().len(), 1);
        assert_eq!(reg.all_plugins()[0].name, "my-plugin");
        assert_eq!(reg.all_plugins()[0].is_builtin, Some(false));
        assert!(reg.is_enabled("my-plugin"));
    }

    #[test]
    fn test_load_plugin_duplicate_skipped() {
        let mut reg = PluginRegistry::new();
        reg.load_plugin(make_manifest("dupe")).unwrap();
        reg.load_plugin(make_manifest("dupe")).unwrap();

        assert_eq!(reg.all_plugins().len(), 1);
    }

    #[test]
    fn test_enable_disable() {
        let mut reg = PluginRegistry::new();
        reg.load_plugin(make_manifest("toggler")).unwrap();

        assert!(reg.is_enabled("toggler"));
        reg.disable("toggler");
        assert!(!reg.is_enabled("toggler"));
        assert!(reg.get_enabled_plugins().is_empty());

        reg.enable("toggler");
        assert!(reg.is_enabled("toggler"));
        assert_eq!(reg.get_enabled_plugins().len(), 1);
    }

    #[test]
    fn test_enable_nonexistent_is_noop() {
        let mut reg = PluginRegistry::new();
        reg.enable("ghost");
        assert!(!reg.is_enabled("ghost"));
    }

    #[test]
    fn test_is_builtin_plugin_id() {
        assert!(PluginRegistry::is_builtin_plugin_id("foo@builtin"));
        assert!(!PluginRegistry::is_builtin_plugin_id("foo@marketplace"));
        assert!(!PluginRegistry::is_builtin_plugin_id("foo"));
    }

    #[test]
    fn test_mixed_plugins() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(make_builtin("builtin-one"));
        reg.load_plugin(make_manifest("loaded-one")).unwrap();
        reg.load_plugin(make_manifest("loaded-two")).unwrap();

        assert_eq!(reg.all_plugins().len(), 3);
        assert_eq!(reg.get_enabled_plugins().len(), 3);

        reg.disable("loaded-two");
        assert_eq!(reg.get_enabled_plugins().len(), 2);
    }

    #[test]
    fn test_get_enabled_only_returns_enabled() {
        let mut reg = PluginRegistry::new();
        reg.load_plugin(make_manifest("a")).unwrap();
        reg.load_plugin(make_manifest("b")).unwrap();
        reg.load_plugin(make_manifest("c")).unwrap();
        reg.disable("b");

        let enabled: Vec<&str> = reg
            .get_enabled_plugins()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert!(enabled.contains(&"a"));
        assert!(!enabled.contains(&"b"));
        assert!(enabled.contains(&"c"));
    }
}
