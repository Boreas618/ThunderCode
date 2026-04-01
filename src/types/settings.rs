//! Basic settings types.
//!
//! Full settings implementation lives in the thundercode-config crate;
//! these are the type definitions needed by other crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// SettingsJson
// ============================================================================

/// The top-level settings.json structure.
///
/// This is a minimal representation -- full validation and defaults
/// are handled by the config crate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SettingsJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_model: Option<ModelSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    /// Additional unrecognized fields for forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ============================================================================
// PermissionsSettings
// ============================================================================

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PermissionsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_directories: Option<Vec<String>>,
}

// ============================================================================
// ModelSetting
// ============================================================================

/// A model setting can be a simple string or a structured object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelSetting {
    /// Just a model name string.
    Name(String),
    /// Structured model configuration.
    Config(ModelConfig),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

// ============================================================================
// HooksSettings
// ============================================================================

/// Hooks configuration -- maps hook event names to hook definitions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HooksSettings {
    #[serde(flatten)]
    pub hooks: HashMap<String, Vec<HookDefinition>>,
}

/// A single hook definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookDefinition {
    /// Shell command to execute.
    pub command: String,
    /// Optional glob/pattern matcher for tool names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// Timeout in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

// ============================================================================
// SettingSource
// ============================================================================

/// Where a setting value originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
}
