//! Permission rule configuration types for settings files.
//!
//! Ported from ref/utils/settings/types.ts` (`PermissionsSchema`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// PermissionsSettings
// ============================================================================

/// Permission rules as stored in settings.json files.
///
/// Each field contains string-encoded rules like `"Bash(npm run:*)"`.
/// Rule validation and matching live in the thundercode-permissions crate;
/// this struct is purely the serialisation shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionsSettings {
    /// List of permission rules for allowed operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,

    /// List of permission rules for denied operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,

    /// List of permission rules that should always prompt for confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<Vec<String>>,

    /// Default permission mode when ThunderCode needs access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,

    /// Disable the ability to bypass permission prompts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_bypass_permissions_mode: Option<String>,

    /// Additional directories to include in the permission scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_directories: Option<Vec<String>>,

    /// Extra fields for forward compatibility (`.passthrough()` in Zod).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl PermissionsSettings {
    /// Return `true` if this struct has no rules and no mode set.
    pub fn is_empty(&self) -> bool {
        self.allow.as_ref().map_or(true, Vec::is_empty)
            && self.deny.as_ref().map_or(true, Vec::is_empty)
            && self.ask.as_ref().map_or(true, Vec::is_empty)
            && self.default_mode.is_none()
            && self.additional_directories
                .as_ref()
                .map_or(true, Vec::is_empty)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_permissions_full() {
        let json = r#"{
            "allow": ["Bash(npm run:*)", "Read"],
            "deny": ["Bash(rm -rf:*)"],
            "ask": ["Write(*.rs)"],
            "defaultMode": "default",
            "additionalDirectories": ["/tmp/extra"]
        }"#;
        let p: PermissionsSettings = serde_json::from_str(json).unwrap();
        assert_eq!(p.allow.as_ref().unwrap().len(), 2);
        assert_eq!(p.deny.as_ref().unwrap().len(), 1);
        assert_eq!(p.ask.as_ref().unwrap().len(), 1);
        assert_eq!(p.default_mode.as_deref(), Some("default"));
        assert_eq!(p.additional_directories.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn empty_permissions() {
        let p = PermissionsSettings::default();
        assert!(p.is_empty());
    }

    #[test]
    fn unknown_permissions_fields_preserved() {
        let json = r#"{"futureRule": "something"}"#;
        let p: PermissionsSettings = serde_json::from_str(json).unwrap();
        assert!(p.extra.contains_key("futureRule"));
    }
}
