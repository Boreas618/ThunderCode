//! Main settings struct -- the comprehensive `SettingsJson`.
//!
//! Ported from ref/utils/settings/types.ts` (`SettingsSchema`).
//! Every field is `Option<T>` and serde is configured for maximum
//! tolerance: unknown fields are collected into `extra` via
//! `#[serde(flatten)]`, and deserialization never fails on missing
//! or unrecognised keys.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::permissions_config::PermissionsSettings;

// ============================================================================
// Top-level SettingsJson
// ============================================================================

/// The top-level settings.json structure.
///
/// This corresponds to the Zod `SettingsSchema` in the TypeScript reference.
/// All fields are optional -- absent keys use runtime defaults.
/// Unknown keys are preserved in `extra` for forward compatibility
/// (mirrors `.passthrough()` in Zod).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsJson {
    // -- JSON Schema reference ---------------------------------------------------
    /// JSON Schema URL (`$schema` key in the file).
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    // -- Authentication / credentials -------------------------------------------
    /// Path to a script that outputs authentication values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_helper: Option<String>,

    /// Path to a script that outputs OpenTelemetry headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otel_headers_helper: Option<String>,

    // -- Model ------------------------------------------------------------------
    /// Override the default model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Allowlist of models that users can select (enterprise).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_models: Option<Vec<String>>,

    /// Override mapping from model ID to provider-specific model ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_overrides: Option<HashMap<String, String>>,

    /// Advisor model for the server-side advisor tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisor_model: Option<String>,

    /// Persisted effort level for supported models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_level: Option<EffortLevel>,

    /// When true, fast mode is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_mode: Option<bool>,

    /// When true, fast mode does not persist across sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_mode_per_session_opt_in: Option<bool>,

    /// When false or absent, thinking is disabled. When true, thinking is
    /// enabled automatically for supported models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_thinking_enabled: Option<bool>,

    // -- Permissions ------------------------------------------------------------
    /// Tool usage permissions configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionsSettings>,

    // -- Hooks ------------------------------------------------------------------
    /// Custom commands to run before/after tool executions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksSettings>,

    /// Disable all hooks and statusLine execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_all_hooks: Option<bool>,

    /// When true (and set in managed settings), only hooks from managed settings run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_managed_hooks_only: Option<bool>,

    /// Allowlist of URL patterns HTTP hooks may target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_http_hook_urls: Option<Vec<String>>,

    /// Allowlist of env var names HTTP hooks may interpolate into headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_hook_allowed_env_vars: Option<Vec<String>>,

    // -- Environment variables --------------------------------------------------
    /// Environment variables to set for ThunderCode sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    // -- Output / theme ---------------------------------------------------------
    /// Controls the output style for assistant responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_style: Option<String>,

    /// Preferred language for AI responses and voice dictation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Whether to disable syntax highlighting in diffs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_highlighting_disabled: Option<bool>,

    /// Whether to show tips in the spinner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_tips_enabled: Option<bool>,

    /// Customize spinner verbs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_verbs: Option<SpinnerVerbsConfig>,

    /// Override spinner tips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_tips_override: Option<SpinnerTipsOverride>,

    /// Reduce or disable animations for accessibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefers_reduced_motion: Option<bool>,

    /// Show thinking summaries in the transcript view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_thinking_summaries: Option<bool>,

    // -- MCP servers ------------------------------------------------------------
    /// Whether to automatically approve all MCP servers in the project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_all_project_mcp_servers: Option<bool>,

    /// List of approved MCP servers from .mcp.json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_mcpjson_servers: Option<Vec<String>>,

    /// List of rejected MCP servers from .mcp.json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcpjson_servers: Option<Vec<String>>,

    /// Enterprise allowlist of MCP servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mcp_servers: Option<Vec<AllowedMcpServerEntry>>,

    /// Enterprise denylist of MCP servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied_mcp_servers: Option<Vec<DeniedMcpServerEntry>>,

    /// Only read MCP allowlist policy from managed settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_managed_mcp_servers_only: Option<bool>,

    // -- Plugins ----------------------------------------------------------------
    /// Enabled plugins using plugin-id@marketplace-id format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_plugins: Option<HashMap<String, serde_json::Value>>,

    /// Per-plugin configuration including MCP server user configs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_configs: Option<HashMap<String, PluginConfig>>,

    /// Additional marketplaces for this repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_known_marketplaces: Option<HashMap<String, serde_json::Value>>,

    /// Enterprise strict list of allowed marketplace sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_known_marketplaces: Option<Vec<serde_json::Value>>,

    /// Enterprise blocklist of marketplace sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_marketplaces: Option<Vec<serde_json::Value>>,

    /// Plugin-only customization lock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_plugin_only_customization: Option<serde_json::Value>,

    /// Custom message to append to plugin trust warning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_trust_message: Option<String>,

    // -- Worktree ---------------------------------------------------------------
    /// Git worktree configuration for --worktree flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeConfig>,

    // -- Shell ------------------------------------------------------------------
    /// Default shell for input-box ! commands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_shell: Option<DefaultShell>,

    // -- Agent ------------------------------------------------------------------
    /// Name of an agent (built-in or custom) to use for the main thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// When false, prompt suggestions are disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_suggestion_enabled: Option<bool>,

    /// When true, the plan-approval dialog offers a "clear context" option.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_clear_context_on_plan_accept: Option<bool>,

    // -- Permissions bypass / auto mode ----------------------------------------
    /// Whether the user has accepted the bypass permissions mode dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_dangerous_mode_permission_prompt: Option<bool>,

    /// Whether the user has accepted the auto mode opt-in dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_auto_permission_prompt: Option<bool>,

    /// Whether plan mode uses auto mode semantics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_auto_mode_during_plan: Option<bool>,

    /// Auto mode classifier prompt customization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_mode: Option<AutoModeConfig>,

    /// Disable auto mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_auto_mode: Option<String>,

    /// Only use permission rules from managed settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_managed_permission_rules_only: Option<bool>,

    // -- Memory -----------------------------------------------------------------
    /// Enable auto-memory for this project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_memory_enabled: Option<bool>,

    /// Custom directory path for auto-memory storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_memory_directory: Option<String>,

    /// Enable background memory consolidation (auto-dream).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_dream_enabled: Option<bool>,

    // -- Git / attribution ------------------------------------------------------
    /// Customize attribution text for commits and PRs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<AttributionConfig>,

    /// Deprecated: Use attribution instead. Whether to include co-authored by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_co_authored_by: Option<bool>,

    /// Include built-in git instructions in system prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_git_instructions: Option<bool>,

    // -- File / context ---------------------------------------------------------
    /// Custom file suggestion configuration for @ mentions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_suggestion: Option<FileSuggestionConfig>,

    /// Whether file picker should respect .gitignore files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respect_gitignore: Option<bool>,

    /// Glob patterns of RULES.md files to exclude from loading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_md_excludes: Option<Vec<String>>,

    // -- Cleanup / retention ----------------------------------------------------
    /// Number of days to retain chat transcripts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_period_days: Option<u32>,

    // -- Login / enterprise admin -----------------------------------------------
    /// Force a specific login method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_login_method: Option<ForceLoginMethod>,

    /// Organization UUID to use for OAuth login.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_login_org_uuid: Option<String>,

    /// Minimum version -- prevents downgrades.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<String>,

    /// Company announcements to display at startup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_announcements: Option<Vec<String>>,

    // -- Sandbox ----------------------------------------------------------------
    /// Sandbox settings (enabled, network, filesystem, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxSettings>,

    // -- Channels ---------------------------------------------------------------
    /// Teams/Enterprise opt-in for channel notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels_enabled: Option<bool>,

    /// Teams/Enterprise allowlist of channel plugins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_channel_plugins: Option<Vec<ChannelPlugin>>,

    // -- Web fetch ---------------------------------------------------------------
    /// Skip the WebFetch blocklist check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_web_fetch_preflight: Option<bool>,

    // -- Status line ------------------------------------------------------------
    /// Custom status line display configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_line: Option<StatusLineConfig>,

    // -- Plans ------------------------------------------------------------------
    /// Custom directory for plan files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_directory: Option<String>,

    // -- Remote -----------------------------------------------------------------
    /// Remote session configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteConfig>,

    // -- Auto updates -----------------------------------------------------------
    /// Release channel for auto-updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_updates_channel: Option<AutoUpdatesChannel>,

    // -- SSH configs -----------------------------------------------------------
    /// SSH connection configurations for remote environments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_configs: Option<Vec<SshConfig>>,

    // -- Terminal ---------------------------------------------------------------
    /// Whether /rename updates the terminal tab title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_title_from_rename: Option<bool>,

    // -- Feedback ---------------------------------------------------------------
    /// Probability (0-1) that the session quality survey appears.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_survey_rate: Option<f64>,

    // -- Catch-all for unknown fields ------------------------------------------
    /// Additional unrecognised fields for forward compatibility.
    /// Mirrors `.passthrough()` in the TypeScript Zod schema.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ============================================================================
// Nested setting types
// ============================================================================

/// Effort level for supported models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    Max,
}

/// Default shell for input-box `!` commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultShell {
    Bash,
    Powershell,
}

/// Force a specific login method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForceLoginMethod {
    Authenticated,
    Console,
}

/// Release channel for auto-updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoUpdatesChannel {
    Latest,
    Stable,
}

/// Hooks configuration -- maps hook event names to arrays of hook matchers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksSettings {
    #[serde(flatten)]
    pub events: HashMap<String, Vec<HookMatcher>>,
}

/// A single hook matcher within a hook event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookMatcher {
    /// Optional tool-name matcher pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,

    /// The hook commands to run when the matcher applies.
    pub hooks: Vec<HookCommand>,
}

/// A single hook command -- either a bash command or an HTTP hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HookCommand {
    /// A shell command hook.
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
    },
    /// An HTTP webhook.
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        allowed_env_vars: Option<Vec<String>>,
    },
    /// An agent prompt hook.
    #[serde(rename = "prompt")]
    Prompt {
        prompt: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
}

/// Git worktree configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorktreeConfig {
    /// Directories to symlink from main repository to worktrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_directories: Option<Vec<String>>,

    /// Directories for git sparse-checkout (cone mode) in worktrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_paths: Option<Vec<String>>,
}

/// Custom file suggestion configuration for @ mentions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSuggestionConfig {
    pub r#type: String,
    pub command: String,
}

/// Attribution configuration for commits and PRs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AttributionConfig {
    /// Attribution text for git commits. Empty string hides attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,

    /// Attribution text for pull request descriptions. Empty string hides attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<String>,
}

/// Custom status line display configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLineConfig {
    pub r#type: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<i32>,
}

/// Spinner verbs configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpinnerVerbsConfig {
    pub mode: SpinnerVerbsMode,
    pub verbs: Vec<String>,
}

/// Mode for spinner verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpinnerVerbsMode {
    Append,
    Replace,
}

/// Spinner tips override.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpinnerTipsOverride {
    #[serde(default)]
    pub exclude_default: bool,
    pub tips: Vec<String>,
}

impl Default for SpinnerTipsOverride {
    fn default() -> Self {
        Self {
            exclude_default: false,
            tips: Vec::new(),
        }
    }
}

/// Allowed MCP server entry in enterprise allowlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedMcpServerEntry {
    /// Name of the MCP server that users are allowed to configure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    /// Command array [command, ...args] to match exactly for allowed stdio servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_command: Option<Vec<String>>,

    /// URL pattern with wildcard support for allowed remote MCP servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
}

/// Denied MCP server entry in enterprise denylist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeniedMcpServerEntry {
    /// Name of the MCP server that is explicitly blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    /// Command array for blocked stdio servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_command: Option<Vec<String>>,

    /// URL pattern for blocked remote MCP servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
}

/// Per-plugin configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginConfig {
    /// User configuration values for MCP servers keyed by server name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, HashMap<String, serde_json::Value>>>,

    /// Non-sensitive option values from plugin manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// Sandbox settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SandboxSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_if_unavailable: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_unsandboxed_commands: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_violations: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_commands: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_allow_bash_if_sandboxed: Option<bool>,

    /// Extra fields for forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Auto mode classifier configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct AutoModeConfig {
    /// Rules for the auto mode classifier allow section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,

    /// Rules for the auto mode classifier deny section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_deny: Option<Vec<String>>,

    /// Entries for the auto mode classifier environment section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Vec<String>>,
}

/// Channel plugin reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPlugin {
    pub marketplace: String,
    pub plugin: String,
}

/// Remote session configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteConfig {
    /// Default environment ID to use for remote sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_environment_id: Option<String>,
}

/// SSH connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfig {
    /// Unique identifier for this SSH config.
    pub id: String,

    /// Display name for the SSH connection.
    pub name: String,

    /// SSH host in format "user@hostname" or a host alias.
    pub ssh_host: String,

    /// SSH port (default: 22).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,

    /// Path to SSH identity file (private key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_identity_file: Option<String>,

    /// Default working directory on the remote host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_directory: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_object() {
        let s: SettingsJson = serde_json::from_str("{}").unwrap();
        assert!(s.model.is_none());
        assert!(s.permissions.is_none());
        assert!(s.extra.is_empty());
    }

    #[test]
    fn deserialize_with_known_fields() {
        let json = r#"{"model": "primary-opus-4-0-20250514", "effortLevel": "high"}"#;
        let s: SettingsJson = serde_json::from_str(json).unwrap();
        assert_eq!(s.model.as_deref(), Some("primary-opus-4-0-20250514"));
        assert_eq!(s.effort_level, Some(EffortLevel::High));
    }

    #[test]
    fn unknown_fields_preserved_in_extra() {
        let json = r#"{"model": "opus", "futureField": 42, "anotherNew": true}"#;
        let s: SettingsJson = serde_json::from_str(json).unwrap();
        assert_eq!(s.model.as_deref(), Some("opus"));
        assert_eq!(s.extra.get("futureField"), Some(&serde_json::json!(42)));
        assert_eq!(s.extra.get("anotherNew"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn roundtrip_serialization() {
        let mut s = SettingsJson::default();
        s.model = Some("test-model".into());
        s.fast_mode = Some(true);
        s.effort_level = Some(EffortLevel::Medium);
        let json = serde_json::to_string(&s).unwrap();
        let s2: SettingsJson = serde_json::from_str(&json).unwrap();
        assert_eq!(s2.model, s.model);
        assert_eq!(s2.fast_mode, Some(true));
        assert_eq!(s2.effort_level, Some(EffortLevel::Medium));
    }

    #[test]
    fn deserialize_permissions() {
        let json = r#"{
            "permissions": {
                "allow": ["Bash(npm run:*)"],
                "deny": ["Bash(rm -rf:*)"],
                "defaultMode": "default"
            }
        }"#;
        let s: SettingsJson = serde_json::from_str(json).unwrap();
        let perms = s.permissions.unwrap();
        assert_eq!(perms.allow.as_ref().unwrap().len(), 1);
        assert_eq!(perms.deny.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn deserialize_hooks_settings() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "echo hello"}]
                }]
            }
        }"#;
        let s: SettingsJson = serde_json::from_str(json).unwrap();
        let hooks = s.hooks.unwrap();
        assert!(hooks.events.contains_key("PreToolUse"));
    }

    #[test]
    fn deserialize_schema_field() {
        let json = r#"{"$schema": "https://json.schemastore.org/primary-code-settings.json"}"#;
        let s: SettingsJson = serde_json::from_str(json).unwrap();
        assert!(s.schema.is_some());
    }
}
