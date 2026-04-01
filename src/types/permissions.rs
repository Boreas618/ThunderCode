//! Permission types -- modes, rules, decisions, and classifier results.
//!
//! Ported from ref/types/permissions.ts (100% fidelity).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Permission Modes
// ============================================================================

/// The external (user-addressable) permission modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalPermissionMode {
    AcceptEdits,
    BypassPermissions,
    Default,
    DontAsk,
    Plan,
}

/// All permission modes including internal-only variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    AcceptEdits,
    BypassPermissions,
    Default,
    DontAsk,
    Plan,
    /// Internal only -- used by the auto-approve classifier.
    Auto,
    /// Internal only -- subagent bubbles permission to parent.
    Bubble,
}

impl From<ExternalPermissionMode> for PermissionMode {
    fn from(mode: ExternalPermissionMode) -> Self {
        match mode {
            ExternalPermissionMode::AcceptEdits => PermissionMode::AcceptEdits,
            ExternalPermissionMode::BypassPermissions => PermissionMode::BypassPermissions,
            ExternalPermissionMode::Default => PermissionMode::Default,
            ExternalPermissionMode::DontAsk => PermissionMode::DontAsk,
            ExternalPermissionMode::Plan => PermissionMode::Plan,
        }
    }
}

/// All external permission mode values for runtime validation.
pub const EXTERNAL_PERMISSION_MODES: &[ExternalPermissionMode] = &[
    ExternalPermissionMode::AcceptEdits,
    ExternalPermissionMode::BypassPermissions,
    ExternalPermissionMode::Default,
    ExternalPermissionMode::DontAsk,
    ExternalPermissionMode::Plan,
];

// ============================================================================
// Permission Behaviors
// ============================================================================

/// The three possible permission behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

// ============================================================================
// Permission Rules
// ============================================================================

/// Where a permission rule originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

/// The value of a permission rule -- specifies which tool and optional content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRuleValue {
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

/// A permission rule with its source and behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub rule_behavior: PermissionBehavior,
    pub rule_value: PermissionRuleValue,
}

// ============================================================================
// Permission Updates
// ============================================================================

/// Where a permission update should be persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionUpdateDestination {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    Session,
    CliArg,
}

/// Update operations for permission configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionUpdate {
    #[serde(rename = "addRules")]
    AddRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    #[serde(rename = "replaceRules")]
    ReplaceRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    #[serde(rename = "removeRules")]
    RemoveRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    #[serde(rename = "setMode")]
    SetMode {
        destination: PermissionUpdateDestination,
        mode: ExternalPermissionMode,
    },
    #[serde(rename = "addDirectories")]
    AddDirectories {
        destination: PermissionUpdateDestination,
        directories: Vec<String>,
    },
    #[serde(rename = "removeDirectories")]
    RemoveDirectories {
        destination: PermissionUpdateDestination,
        directories: Vec<String>,
    },
}

// ============================================================================
// Working Directories
// ============================================================================

/// Source of an additional working directory permission (same as PermissionRuleSource).
pub type WorkingDirectorySource = PermissionRuleSource;

/// An additional directory included in permission scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub source: WorkingDirectorySource,
}

// ============================================================================
// Permission Decisions & Results
// ============================================================================

/// Minimal command shape for permission metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCommandMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Extra properties for forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Metadata attached to permission decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionMetadata {
    pub command: PermissionCommandMetadata,
}

/// Metadata for a pending classifier check that will run asynchronously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClassifierCheck {
    pub command: String,
    pub cwd: String,
    pub descriptions: Vec<String>,
}

/// Result when permission is granted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAllowDecision {
    pub behavior: AllowBehaviorTag,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_modified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<PermissionDecisionReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_feedback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
}

/// Result when user should be prompted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAskDecision {
    pub behavior: AskBehaviorTag,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<PermissionDecisionReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<PermissionUpdate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PermissionMetadata>,
    /// If true, triggered by a deprecated bash security check for misparse-prone patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_bash_security_check_for_misparsing: Option<bool>,
    /// If set, an allow classifier check should be run asynchronously.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_classifier_check: Option<PendingClassifierCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
}

/// Result when permission is denied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDenyDecision {
    pub behavior: DenyBehaviorTag,
    pub message: String,
    pub decision_reason: PermissionDecisionReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

// Behavior tag types for cleaner serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllowBehaviorTag {
    #[serde(rename = "allow")]
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskBehaviorTag {
    #[serde(rename = "ask")]
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenyBehaviorTag {
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassthroughBehaviorTag {
    #[serde(rename = "passthrough")]
    Passthrough,
}

/// A permission decision -- allow, ask, or deny.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum PermissionDecision {
    #[serde(rename = "allow")]
    Allow {
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_modified: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        accept_feedback: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
    },
    #[serde(rename = "ask")]
    Ask {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<PermissionMetadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_bash_security_check_for_misparsing: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
    },
    #[serde(rename = "deny")]
    Deny {
        message: String,
        decision_reason: PermissionDecisionReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
}

/// Permission result with additional passthrough option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum PermissionResult {
    #[serde(rename = "allow")]
    Allow {
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_modified: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        accept_feedback: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
    },
    #[serde(rename = "ask")]
    Ask {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<PermissionMetadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_bash_security_check_for_misparsing: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<crate::types::content::ContentBlockParam>>,
    },
    #[serde(rename = "deny")]
    Deny {
        message: String,
        decision_reason: PermissionDecisionReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
    #[serde(rename = "passthrough")]
    Passthrough {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
    },
}

impl PermissionResult {
    /// Convenience constructor for a simple allow result.
    pub fn allow(updated_input: Option<serde_json::Value>) -> Self {
        PermissionResult::Allow {
            updated_input,
            user_modified: None,
            decision_reason: None,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        }
    }

    /// Convenience constructor for a simple deny result.
    pub fn deny(message: impl Into<String>, reason: PermissionDecisionReason) -> Self {
        PermissionResult::Deny {
            message: message.into(),
            decision_reason: reason,
            tool_use_id: None,
        }
    }

    /// Get the behavior tag.
    pub fn behavior(&self) -> PermissionBehavior {
        match self {
            PermissionResult::Allow { .. } => PermissionBehavior::Allow,
            PermissionResult::Ask { .. } => PermissionBehavior::Ask,
            PermissionResult::Deny { .. } => PermissionBehavior::Deny,
            // passthrough maps to ask in terms of behavior
            PermissionResult::Passthrough { .. } => PermissionBehavior::Ask,
        }
    }
}

// ============================================================================
// Permission Decision Reason
// ============================================================================

/// Explanation of why a permission decision was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PermissionDecisionReason {
    #[serde(rename = "rule")]
    Rule { rule: PermissionRule },

    #[serde(rename = "mode")]
    Mode { mode: PermissionMode },

    #[serde(rename = "subcommandResults")]
    SubcommandResults {
        /// Map of subcommand to its permission result.
        reasons: HashMap<String, PermissionResult>,
    },

    #[serde(rename = "permissionPromptTool")]
    PermissionPromptTool {
        permission_prompt_tool_name: String,
        tool_result: serde_json::Value,
    },

    #[serde(rename = "hook")]
    Hook {
        hook_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hook_source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    #[serde(rename = "asyncAgent")]
    AsyncAgent { reason: String },

    #[serde(rename = "sandboxOverride")]
    SandboxOverride { reason: SandboxOverrideReason },

    #[serde(rename = "classifier")]
    Classifier { classifier: String, reason: String },

    #[serde(rename = "workingDir")]
    WorkingDir { reason: String },

    #[serde(rename = "safetyCheck")]
    SafetyCheck {
        reason: String,
        /// When true, auto mode lets the classifier evaluate this.
        classifier_approvable: bool,
    },

    #[serde(rename = "other")]
    Other { reason: String },
}

/// Reason for a sandbox override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxOverrideReason {
    ExcludedCommand,
    DangerouslyDisableSandbox,
}

// ============================================================================
// Bash Classifier Types
// ============================================================================

/// Result from the bash command classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierResult {
    pub matches: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_description: Option<String>,
    pub confidence: ClassifierConfidence,
    pub reason: String,
}

/// Confidence level for a classifier result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassifierConfidence {
    High,
    Medium,
    Low,
}

/// Classifier behavior: deny, ask, or allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassifierBehavior {
    Deny,
    Ask,
    Allow,
}

/// Token usage from a classifier API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// Result from the YOLO/auto-mode classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoloClassifierResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    pub should_block: bool,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<bool>,
    /// API returned "prompt is too long".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_too_long: Option<bool>,
    /// The model used for this classifier call.
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ClassifierUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_lengths: Option<ClassifierPromptLengths>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_dump_path: Option<String>,
    /// Which classifier stage produced the final decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<ClassifierStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage1_usage: Option<ClassifierUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage1_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage1_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage1_msg_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage2_usage: Option<ClassifierUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage2_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage2_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage2_msg_id: Option<String>,
}

/// Classifier prompt component lengths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierPromptLengths {
    pub system_prompt: usize,
    pub tool_calls: usize,
    pub user_prompts: usize,
}

/// Which stage of the 2-stage classifier ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassifierStage {
    Fast,
    Thinking,
}

// ============================================================================
// Permission Explainer Types
// ============================================================================

/// Risk level assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    LOW,
    MEDIUM,
    HIGH,
}

/// Human-readable explanation of a permission decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionExplanation {
    pub risk_level: RiskLevel,
    pub explanation: String,
    pub reasoning: String,
    pub risk: String,
}

// ============================================================================
// Tool Permission Context
// ============================================================================

/// Mapping of permission rules by their source.
/// Keys are `PermissionRuleSource` variants, values are rule content strings.
pub type ToolPermissionRulesBySource = HashMap<String, Vec<String>>;

/// Context needed for permission checking in tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: HashMap<String, AdditionalWorkingDirectory>,
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
    pub is_bypass_permissions_mode_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_auto_mode_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripped_dangerous_rules: Option<ToolPermissionRulesBySource>,
    /// When true, permission prompts are auto-denied (e.g., background agents).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_avoid_permission_prompts: Option<bool>,
    /// When true, automated checks are awaited before showing the permission dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_automated_checks_before_dialog: Option<bool>,
    /// Stores the permission mode before model-initiated plan mode entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_plan_mode: Option<PermissionMode>,
}

impl Default for ToolPermissionContext {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            additional_working_directories: HashMap::new(),
            always_allow_rules: HashMap::new(),
            always_deny_rules: HashMap::new(),
            always_ask_rules: HashMap::new(),
            is_bypass_permissions_mode_available: false,
            is_auto_mode_available: None,
            stripped_dangerous_rules: None,
            should_avoid_permission_prompts: None,
            await_automated_checks_before_dialog: None,
            pre_plan_mode: None,
        }
    }
}
