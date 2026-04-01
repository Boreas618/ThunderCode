//! Built-in plugin definitions.
//!
//! Returns the set of plugins that ship with the CLI and appear in the
//! `/plugin` UI for users to enable/disable. Currently the list is empty --
//! this is scaffolding for migrating bundled skills that should be
//! user-toggleable.
//!
//! Ported from ref/plugins/bundled/index.ts`.

use crate::types::plugin::BuiltinPluginDefinition;

// ---------------------------------------------------------------------------
// get_builtin_plugins
// ---------------------------------------------------------------------------

/// Return all built-in plugin definitions.
///
/// Called during CLI startup to register these plugins in the
/// [`PluginRegistry`](crate::plugins::registry::PluginRegistry). New built-in plugins
/// should be added to the vector returned here.
pub fn get_builtin_plugins() -> Vec<BuiltinPluginDefinition> {
    // No built-in plugins registered yet -- this is the scaffolding for
    // migrating bundled skills that should be user-toggleable.
    //
    // To add a new built-in plugin, push a BuiltinPluginDefinition:
    //
    //   vec![
    //       BuiltinPluginDefinition {
    //           name: "example-plugin".to_string(),
    //           description: "An example built-in plugin".to_string(),
    //           version: Some("1.0.0".to_string()),
    //           default_enabled: Some(true),
    //       },
    //   ]
    Vec::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::registry::PluginRegistry;

    #[test]
    fn test_get_builtin_plugins_returns_empty() {
        let plugins = get_builtin_plugins();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_register_builtins_in_registry() {
        let mut registry = PluginRegistry::new();
        for plugin in get_builtin_plugins() {
            registry.register_builtin(plugin);
        }
        // With an empty list, registry should still be empty.
        assert!(registry.all_plugins().is_empty());
    }

    #[test]
    fn test_builtin_definition_shape() {
        // Verify we can construct a definition and register it.
        let def = BuiltinPluginDefinition {
            name: "test-builtin".to_string(),
            description: "A test built-in plugin".to_string(),
            version: Some("0.1.0".to_string()),
            default_enabled: Some(true),
        };

        let mut registry = PluginRegistry::new();
        registry.register_builtin(def);
        assert_eq!(registry.all_plugins().len(), 1);
        assert!(registry.is_enabled("test-builtin"));
    }
}
