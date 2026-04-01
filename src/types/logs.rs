//! Log and transcript types.
//!
//! Ported from ref/types/logs.ts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::ids::AgentId;
use crate::types::messages::Message;

// ============================================================================
// SerializedMessage
// ============================================================================

/// A message with serialization metadata for transcript storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMessage {
    /// The underlying message.
    #[serde(flatten)]
    pub message: Message,
    pub cwd: String,
    pub user_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    pub session_id: String,
    pub timestamp: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

// ============================================================================
// TranscriptMessage
// ============================================================================

/// A message in the transcript with parent/sidechain metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    #[serde(flatten)]
    pub serialized: SerializedMessage,
    pub parent_uuid: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logical_parent_uuid: Option<Uuid>,
    pub is_sidechain: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
}

// ============================================================================
// LogOption
// ============================================================================

/// A log entry representing a session, displayed in the resume UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOption {
    pub date: String,
    pub messages: Vec<SerializedMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_path: Option<String>,
    pub value: i64,
    pub created: String,
    pub modified: String,
    pub first_prompt: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    pub is_sidechain: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_lite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_setting: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_teammate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_uuid: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SessionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_session: Option<PersistedWorktreeSession>,
}

/// Session mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    Coordinator,
    Normal,
}

// ============================================================================
// Metadata Messages (entries appended to transcript)
// ============================================================================

/// AI-generated or user-set session summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMessage {
    #[serde(rename = "type")]
    pub message_type: SummaryMessageType,
    pub leaf_uuid: Uuid,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SummaryMessageType {
    #[serde(rename = "summary")]
    Summary,
}

/// User-set custom session title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTitleMessage {
    #[serde(rename = "type")]
    pub message_type: CustomTitleMessageType,
    pub session_id: Uuid,
    pub custom_title: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CustomTitleMessageType {
    #[serde(rename = "custom-title")]
    CustomTitle,
}

/// AI-generated session title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTitleMessage {
    #[serde(rename = "type")]
    pub message_type: AiTitleMessageType,
    pub session_id: Uuid,
    pub ai_title: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AiTitleMessageType {
    #[serde(rename = "ai-title")]
    AiTitle,
}

/// Last prompt in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPromptMessage {
    #[serde(rename = "type")]
    pub message_type: LastPromptMessageType,
    pub session_id: Uuid,
    pub last_prompt: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LastPromptMessageType {
    #[serde(rename = "last-prompt")]
    LastPrompt,
}

/// Periodic summary of what the agent is currently doing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummaryMessage {
    #[serde(rename = "type")]
    pub message_type: TaskSummaryMessageType,
    pub session_id: Uuid,
    pub summary: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TaskSummaryMessageType {
    #[serde(rename = "task-summary")]
    TaskSummary,
}

/// Session tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMessage {
    #[serde(rename = "type")]
    pub message_type: TagMessageType,
    pub session_id: Uuid,
    pub tag: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TagMessageType {
    #[serde(rename = "tag")]
    Tag,
}

/// Agent custom name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNameMessage {
    #[serde(rename = "type")]
    pub message_type: AgentNameMessageType,
    pub session_id: Uuid,
    pub agent_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AgentNameMessageType {
    #[serde(rename = "agent-name")]
    AgentName,
}

/// Agent custom color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentColorMessage {
    #[serde(rename = "type")]
    pub message_type: AgentColorMessageType,
    pub session_id: Uuid,
    pub agent_color: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AgentColorMessageType {
    #[serde(rename = "agent-color")]
    AgentColor,
}

/// Agent definition setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettingMessage {
    #[serde(rename = "type")]
    pub message_type: AgentSettingMessageType,
    pub session_id: Uuid,
    pub agent_setting: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AgentSettingMessageType {
    #[serde(rename = "agent-setting")]
    AgentSetting,
}

/// PR link message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRLinkMessage {
    #[serde(rename = "type")]
    pub message_type: PRLinkMessageType,
    pub session_id: Uuid,
    pub pr_number: u64,
    pub pr_url: String,
    pub pr_repository: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PRLinkMessageType {
    #[serde(rename = "pr-link")]
    PrLink,
}

/// Mode entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeEntry {
    #[serde(rename = "type")]
    pub message_type: ModeEntryType,
    pub session_id: Uuid,
    pub mode: SessionMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ModeEntryType {
    #[serde(rename = "mode")]
    Mode,
}

// ============================================================================
// Worktree State
// ============================================================================

/// Worktree session state persisted to the transcript for resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedWorktreeSession {
    pub original_cwd: String,
    pub worktree_path: String,
    pub worktree_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_head_commit: Option<String>,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_based: Option<bool>,
}

/// Worktree state entry in the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeStateEntry {
    #[serde(rename = "type")]
    pub message_type: WorktreeStateEntryType,
    pub session_id: Uuid,
    pub worktree_session: Option<PersistedWorktreeSession>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorktreeStateEntryType {
    #[serde(rename = "worktree-state")]
    WorktreeState,
}

// ============================================================================
// Attribution
// ============================================================================

/// Per-file attribution state tracking contributions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAttributionState {
    pub content_hash: String,
    pub ai_contribution: u64,
    pub mtime: u64,
}

/// Attribution snapshot message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionSnapshotMessage {
    #[serde(rename = "type")]
    pub message_type: AttributionSnapshotMessageType,
    pub message_id: Uuid,
    pub surface: String,
    pub file_states: HashMap<String, FileAttributionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_count_at_last_commit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_prompt_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_prompt_count_at_last_commit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escape_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escape_count_at_last_commit: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AttributionSnapshotMessageType {
    #[serde(rename = "attribution-snapshot")]
    AttributionSnapshot,
}

// ============================================================================
// File History Snapshot
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistorySnapshotMessage {
    #[serde(rename = "type")]
    pub message_type: FileHistorySnapshotMessageType,
    pub message_id: Uuid,
    pub snapshot: serde_json::Value, // FileHistorySnapshot
    pub is_snapshot_update: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FileHistorySnapshotMessageType {
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot,
}

// ============================================================================
// Content Replacement
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReplacementEntry {
    #[serde(rename = "type")]
    pub message_type: ContentReplacementEntryType,
    pub session_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub replacements: Vec<serde_json::Value>, // ContentReplacementRecord
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ContentReplacementEntryType {
    #[serde(rename = "content-replacement")]
    ContentReplacement,
}

// ============================================================================
// Speculation Accept
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculationAcceptMessage {
    #[serde(rename = "type")]
    pub message_type: SpeculationAcceptMessageType,
    pub timestamp: String,
    pub time_saved_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SpeculationAcceptMessageType {
    #[serde(rename = "speculation-accept")]
    SpeculationAccept,
}

// ============================================================================
// Context Collapse
// ============================================================================

/// Persisted context-collapse commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCollapseCommitEntry {
    #[serde(rename = "type")]
    pub message_type: ContextCollapseCommitEntryType,
    pub session_id: Uuid,
    pub collapse_id: String,
    pub summary_uuid: String,
    pub summary_content: String,
    pub summary: String,
    pub first_archived_uuid: String,
    pub last_archived_uuid: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ContextCollapseCommitEntryType {
    #[serde(rename = "marble-origami-commit")]
    MarbleOrigamiCommit,
}

/// Snapshot of the staged queue and spawn trigger state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCollapseSnapshotEntry {
    #[serde(rename = "type")]
    pub message_type: ContextCollapseSnapshotEntryType,
    pub session_id: Uuid,
    pub staged: Vec<StagedCollapse>,
    pub armed: bool,
    pub last_spawn_tokens: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ContextCollapseSnapshotEntryType {
    #[serde(rename = "marble-origami-snapshot")]
    MarbleOrigamiSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedCollapse {
    pub start_uuid: String,
    pub end_uuid: String,
    pub summary: String,
    pub risk: f64,
    pub staged_at: u64,
}

// ============================================================================
// Entry -- the top-level transcript entry union
// ============================================================================

/// All possible transcript entry types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    Transcript(TranscriptMessage),
    Summary(SummaryMessage),
    CustomTitle(CustomTitleMessage),
    AiTitle(AiTitleMessage),
    LastPrompt(LastPromptMessage),
    TaskSummary(TaskSummaryMessage),
    Tag(TagMessage),
    AgentName(AgentNameMessage),
    AgentColor(AgentColorMessage),
    AgentSetting(AgentSettingMessage),
    PRLink(PRLinkMessage),
    FileHistorySnapshot(FileHistorySnapshotMessage),
    AttributionSnapshot(AttributionSnapshotMessage),
    SpeculationAccept(SpeculationAcceptMessage),
    ModeEntry(ModeEntry),
    WorktreeState(WorktreeStateEntry),
    ContentReplacement(ContentReplacementEntry),
    ContextCollapseCommit(ContextCollapseCommitEntry),
    ContextCollapseSnapshot(ContextCollapseSnapshotEntry),
}

// ============================================================================
// Helpers
// ============================================================================

/// Sort logs by modified date (newest first), then created date.
pub fn sort_logs(logs: &mut Vec<LogOption>) {
    logs.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.created.cmp(&a.created))
    });
}
