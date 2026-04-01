//! Command types for slash commands and skills.
//!
//! Ported from ref/types/command.ts.

use serde::{Deserialize, Serialize};

// ============================================================================
// CommandAvailability
// ============================================================================

/// Declares which environments a command is available in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandAvailability {
    /// Available to all authenticated users.
    Authenticated,
}

// ============================================================================
// CommandResultDisplay
// ============================================================================

/// How to display the result of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandResultDisplay {
    Skip,
    System,
    User,
}

// ============================================================================
// ResumeEntrypoint
// ============================================================================

/// How a session resume was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeEntrypoint {
    CliFlag,
    SlashCommandPicker,
    SlashCommandSessionId,
    SlashCommandTitle,
    Fork,
}

// ============================================================================
// CommandBase
// ============================================================================

/// Base fields shared by all command types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBase {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_user_specified_description: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<Vec<CommandAvailability>>,
    /// Defaults to true when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    /// When true, the command is hidden from typeahead/help.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mcp: Option<bool>,
    /// Hint text for command arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    /// Detailed usage scenarios (from the "Skill" spec).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_to_use: Option<String>,
    /// Version of the command/skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether to disable this command from being invoked by models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_model_invocation: Option<bool>,
    /// Whether users can invoke this skill by typing /skill-name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_invocable: Option<bool>,
    /// Where the command was loaded from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_from: Option<CommandLoadedFrom>,
    /// Distinguishes workflow-backed commands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CommandKind>,
    /// If true, command executes immediately without waiting for a stop point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immediate: Option<bool>,
    /// If true, args are redacted from conversation history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_sensitive: Option<bool>,
    /// User-facing name (defaults to `name`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_facing_name: Option<String>,
}

/// Where a command was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandLoadedFrom {
    #[serde(rename = "commands_DEPRECATED")]
    CommandsDeprecated,
    Skills,
    Plugin,
    Managed,
    Bundled,
    Mcp,
}

/// Kind of command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandKind {
    Workflow,
}

// ============================================================================
// Command Type (tagged union)
// ============================================================================

/// The full command type -- base fields plus type-specific data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// A prompt command that sends content to the model.
    #[serde(rename = "prompt")]
    Prompt(PromptCommandData),

    /// A local command that runs code locally.
    #[serde(rename = "local")]
    Local(LocalCommandData),

    /// A local JSX command that renders UI.
    #[serde(rename = "local-jsx")]
    LocalJsx(LocalJsxCommandData),
}

impl Command {
    /// Get the base fields of this command.
    pub fn base(&self) -> &CommandBase {
        match self {
            Command::Prompt(p) => &p.base,
            Command::Local(l) => &l.base,
            Command::LocalJsx(j) => &j.base,
        }
    }

    /// Get the command name.
    pub fn name(&self) -> &str {
        &self.base().name
    }

    /// Get the user-facing name, falling back to `name`.
    pub fn user_facing_name(&self) -> &str {
        self.base()
            .user_facing_name
            .as_deref()
            .unwrap_or(&self.base().name)
    }

    /// Whether the command is enabled (defaults to true).
    pub fn is_enabled(&self) -> bool {
        self.base().is_enabled.unwrap_or(true)
    }
}

// ============================================================================
// PromptCommand
// ============================================================================

/// A prompt command that sends content to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCommandData {
    #[serde(flatten)]
    pub base: CommandBase,
    pub progress_message: String,
    /// Length of command content in characters (for token estimation).
    pub content_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub source: PromptCommandSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_info: Option<PromptCommandPluginInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_non_interactive: Option<bool>,
    /// Hooks to register when this skill is invoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<serde_json::Value>, // HooksSettings -- defined in settings.rs
    /// Base directory for skill resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_root: Option<String>,
    /// Execution context: 'inline' (default) or 'fork' (run as sub-agent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<PromptCommandContext>,
    /// Agent type to use when forked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Effort level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Glob patterns for file paths this skill applies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

/// Source of a prompt command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptCommandSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    Builtin,
    Mcp,
    Plugin,
    Bundled,
}

/// Plugin info attached to a prompt command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCommandPluginInfo {
    pub plugin_manifest: serde_json::Value, // PluginManifest
    pub repository: String,
}

/// Execution context for a prompt command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptCommandContext {
    Inline,
    Fork,
}

// ============================================================================
// LocalCommand
// ============================================================================

/// A local command that runs code locally (non-JSX).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCommandData {
    #[serde(flatten)]
    pub base: CommandBase,
    pub supports_non_interactive: bool,
}

// ============================================================================
// LocalJsxCommand
// ============================================================================

/// A local JSX command that renders interactive UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalJsxCommandData {
    #[serde(flatten)]
    pub base: CommandBase,
}
