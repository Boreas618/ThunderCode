//! Central application state.
//!
//! Ported from ref/state/AppStateStore.ts -- the single `AppState` struct
//! that describes the entire reactive UI surface.  This is held inside a
//! `Store<AppState>` (see [`crate::state::store::Store`]).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::types::permissions::{PermissionMode, ToolPermissionContext};
use crate::types::settings::SettingsJson;
use crate::types::task::TaskStateBase;

use crate::state::notification::Notification;

// ---------------------------------------------------------------------------
// Supporting enums
// ---------------------------------------------------------------------------

/// Which footer pill is focused in arrow-key navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FooterSelection {
    Tasks,
    Bridge,
}

/// Voice input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceMode {
    Off,
    Listening,
    Processing,
}

impl Default for VoiceMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Always-on bridge connection lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BridgeState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// MCP server connection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConnection {
    pub name: String,
    pub status: McpConnectionStatus,
}

/// Status of an MCP server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

/// An agent definition loaded from the agents directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

/// Expanded view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpandedView {
    None,
    Tasks,
    Teammates,
}

impl Default for ExpandedView {
    fn default() -> Self {
        Self::None
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// The central reactive application state.
///
/// This struct is the Rust equivalent of the TypeScript `AppState` type from
/// `ref/state/AppStateStore.ts`.  It is stored inside a
/// [`Store<AppState>`](crate::state::store::Store) and drives the TUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    // ---- Settings & model ------------------------------------------------
    /// Merged settings from all sources.
    pub settings: SettingsJson,
    /// Active model name/alias (empty string = use default).
    pub model: String,
    /// Verbose output mode.
    pub verbose: bool,
    /// Expanded message view.
    pub expanded_view: ExpandedView,

    // ---- UI state --------------------------------------------------------
    /// Index of the selected in-process agent (None = leader).
    pub selected_agent_index: Option<usize>,
    /// Which footer pill is focused.
    pub footer_selection: Option<FooterSelection>,

    // ---- Permission state ------------------------------------------------
    /// Current permission mode.
    pub permission_mode: PermissionMode,
    /// Full permission context for tool checks.
    pub tool_permission_context: ToolPermissionContext,

    // ---- Notifications ---------------------------------------------------
    /// Queued notifications awaiting display.
    pub notifications: Vec<Notification>,
    /// Currently displayed notification (if any).
    pub current_notification: Option<Notification>,

    // ---- Agent state -----------------------------------------------------
    /// Agent name from `--agent` flag or settings.
    pub agent_name: Option<String>,
    /// Agent color for the logo/header.
    pub agent_color: Option<String>,
    /// Count of background tasks running.
    pub background_task_count: usize,

    // ---- Bridge state ----------------------------------------------------
    /// Always-on bridge lifecycle state.
    pub bridge_state: BridgeState,

    // ---- Tasks -----------------------------------------------------------
    /// All active/recent tasks keyed by task ID.
    pub tasks: HashMap<String, TaskStateBase>,
    /// Available agent definitions.
    pub agent_definitions: Vec<AgentDefinition>,
    /// Whether the task list panel is expanded.
    pub task_list_expanded: bool,

    // ---- Media -----------------------------------------------------------
    /// Paths to user-selected images for the next prompt.
    pub selected_images: Vec<String>,
    /// Paths to images pasted from clipboard.
    pub clipboard_images: Vec<String>,

    // ---- Voice -----------------------------------------------------------
    /// Voice input mode.
    pub voice_mode: VoiceMode,

    // ---- MCP -------------------------------------------------------------
    /// Active MCP server connections.
    pub mcp_connections: Vec<McpConnection>,

    // ---- Plugins ---------------------------------------------------------
    /// Names of loaded plugins.
    pub loaded_plugins: Vec<String>,

    // ---- Scrolling -------------------------------------------------------
    /// Indices of messages whose content is expanded in the scroll-back.
    pub expanded_message_indices: HashSet<usize>,
}

// ---------------------------------------------------------------------------
// Default
// ---------------------------------------------------------------------------

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: SettingsJson::default(),
            model: String::new(),
            verbose: false,
            expanded_view: ExpandedView::default(),

            selected_agent_index: None,
            footer_selection: None,

            permission_mode: PermissionMode::Default,
            tool_permission_context: ToolPermissionContext::default(),

            notifications: Vec::new(),
            current_notification: None,

            agent_name: None,
            agent_color: None,
            background_task_count: 0,

            bridge_state: BridgeState::default(),

            tasks: HashMap::new(),
            agent_definitions: Vec::new(),
            task_list_expanded: false,

            selected_images: Vec::new(),
            clipboard_images: Vec::new(),

            voice_mode: VoiceMode::default(),

            mcp_connections: Vec::new(),

            loaded_plugins: Vec::new(),

            expanded_message_indices: HashSet::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience type alias
// ---------------------------------------------------------------------------

/// A reactive store holding the [`AppState`].
pub type AppStateStore = crate::state::store::Store<AppState>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sane_values() {
        let state = AppState::default();
        assert_eq!(state.permission_mode, PermissionMode::Default);
        assert!(!state.verbose);
        assert!(state.tasks.is_empty());
        assert!(state.notifications.is_empty());
        assert_eq!(state.background_task_count, 0);
        assert_eq!(state.bridge_state, BridgeState::Disconnected);
        assert_eq!(state.voice_mode, VoiceMode::Off);
    }

    #[test]
    fn app_state_is_clone_and_partial_eq() {
        let a = AppState::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn in_store() {
        let store = AppStateStore::new(AppState::default());
        store.set_state(|prev| {
            let mut next = prev.clone();
            next.verbose = true;
            next
        });
        assert!(store.get_state().verbose);
    }
}
