//! All keybinding actions that can be triggered by key combinations.
//!
//! Ported from `KEYBINDING_ACTIONS` in `schema.ts`. Every action listed in the
//! TypeScript source is represented here.

use serde::{Deserialize, Serialize};
use std::fmt;

/// All valid keybinding action identifiers.
///
/// Each variant maps to a `"category:action"` string from the TypeScript source.
/// The string representation uses the `category:action` format for serialization
/// and display, matching the original TypeScript identifiers exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeybindingAction {
    // ── App-level actions (Global context) ──────────────────────────────
    /// Interrupt current operation (ctrl+c).
    #[serde(rename = "app:interrupt")]
    AppInterrupt,
    /// Exit the application (ctrl+d).
    #[serde(rename = "app:exit")]
    AppExit,
    /// Toggle the to-do/task list panel.
    #[serde(rename = "app:toggleTodos")]
    AppToggleTodos,
    /// Toggle the transcript viewer.
    #[serde(rename = "app:toggleTranscript")]
    AppToggleTranscript,
    /// Toggle brief mode.
    #[serde(rename = "app:toggleBrief")]
    AppToggleBrief,
    /// Toggle teammate preview.
    #[serde(rename = "app:toggleTeammatePreview")]
    AppToggleTeammatePreview,
    /// Toggle the terminal panel.
    #[serde(rename = "app:toggleTerminal")]
    AppToggleTerminal,
    /// Redraw the terminal screen.
    #[serde(rename = "app:redraw")]
    AppRedraw,
    /// Open global file search.
    #[serde(rename = "app:globalSearch")]
    AppGlobalSearch,
    /// Open quick file open.
    #[serde(rename = "app:quickOpen")]
    AppQuickOpen,

    // ── History navigation ──────────────────────────────────────────────
    /// Open history search (ctrl+r).
    #[serde(rename = "history:search")]
    HistorySearch,
    /// Navigate to previous history entry.
    #[serde(rename = "history:previous")]
    HistoryPrevious,
    /// Navigate to next history entry.
    #[serde(rename = "history:next")]
    HistoryNext,

    // ── Chat input actions ──────────────────────────────────────────────
    /// Cancel current chat input.
    #[serde(rename = "chat:cancel")]
    ChatCancel,
    /// Kill all running agents.
    #[serde(rename = "chat:killAgents")]
    ChatKillAgents,
    /// Cycle through chat modes (e.g., plan/auto).
    #[serde(rename = "chat:cycleMode")]
    ChatCycleMode,
    /// Open the model picker.
    #[serde(rename = "chat:modelPicker")]
    ChatModelPicker,
    /// Toggle fast mode.
    #[serde(rename = "chat:fastMode")]
    ChatFastMode,
    /// Toggle extended thinking.
    #[serde(rename = "chat:thinkingToggle")]
    ChatThinkingToggle,
    /// Submit the current chat message.
    #[serde(rename = "chat:submit")]
    ChatSubmit,
    /// Insert a newline in chat input.
    #[serde(rename = "chat:newline")]
    ChatNewLine,
    /// Undo last edit in chat input.
    #[serde(rename = "chat:undo")]
    ChatUndo,
    /// Open external editor for chat input.
    #[serde(rename = "chat:externalEditor")]
    ChatExternalEditor,
    /// Stash the current input.
    #[serde(rename = "chat:stash")]
    ChatStash,
    /// Paste an image into chat.
    #[serde(rename = "chat:imagePaste")]
    ChatImagePaste,
    /// Open message actions menu.
    #[serde(rename = "chat:messageActions")]
    ChatMessageActions,

    // ── Autocomplete menu actions ───────────────────────────────────────
    /// Accept the current autocomplete suggestion.
    #[serde(rename = "autocomplete:accept")]
    AutocompleteAccept,
    /// Dismiss the autocomplete menu.
    #[serde(rename = "autocomplete:dismiss")]
    AutocompleteDismiss,
    /// Navigate to previous autocomplete suggestion.
    #[serde(rename = "autocomplete:previous")]
    AutocompletePrevious,
    /// Navigate to next autocomplete suggestion.
    #[serde(rename = "autocomplete:next")]
    AutocompleteNext,

    // ── Confirmation dialog actions ─────────────────────────────────────
    /// Accept / confirm yes.
    #[serde(rename = "confirm:yes")]
    ConfirmYes,
    /// Reject / confirm no.
    #[serde(rename = "confirm:no")]
    ConfirmNo,
    /// Navigate to previous item in confirmation list.
    #[serde(rename = "confirm:previous")]
    ConfirmPrevious,
    /// Navigate to next item in confirmation list.
    #[serde(rename = "confirm:next")]
    ConfirmNext,
    /// Move to next field in multi-field dialogs.
    #[serde(rename = "confirm:nextField")]
    ConfirmNextField,
    /// Move to previous field in multi-field dialogs.
    #[serde(rename = "confirm:previousField")]
    ConfirmPreviousField,
    /// Cycle modes in permission/teams dialogs.
    #[serde(rename = "confirm:cycleMode")]
    ConfirmCycleMode,
    /// Toggle a checkbox or option.
    #[serde(rename = "confirm:toggle")]
    ConfirmToggle,
    /// Toggle permission explanation.
    #[serde(rename = "confirm:toggleExplanation")]
    ConfirmToggleExplanation,

    // ── Tabs navigation actions ─────────────────────────────────────────
    /// Navigate to next tab.
    #[serde(rename = "tabs:next")]
    TabsNext,
    /// Navigate to previous tab.
    #[serde(rename = "tabs:previous")]
    TabsPrevious,

    // ── Transcript viewer actions ───────────────────────────────────────
    /// Toggle showing all transcript entries.
    #[serde(rename = "transcript:toggleShowAll")]
    TranscriptToggleShowAll,
    /// Exit the transcript viewer.
    #[serde(rename = "transcript:exit")]
    TranscriptExit,

    // ── History search actions ──────────────────────────────────────────
    /// Navigate to next match in history search.
    #[serde(rename = "historySearch:next")]
    HistorySearchNext,
    /// Accept the current history search result.
    #[serde(rename = "historySearch:accept")]
    HistorySearchAccept,
    /// Cancel history search.
    #[serde(rename = "historySearch:cancel")]
    HistorySearchCancel,
    /// Execute the selected history search result.
    #[serde(rename = "historySearch:execute")]
    HistorySearchExecute,

    // ── Task/agent actions ──────────────────────────────────────────────
    /// Background the current running task.
    #[serde(rename = "task:background")]
    TaskBackground,

    // ── Theme picker actions ────────────────────────────────────────────
    /// Toggle syntax highlighting in theme picker.
    #[serde(rename = "theme:toggleSyntaxHighlighting")]
    ThemeToggleSyntaxHighlighting,

    // ── Help menu actions ───────────────────────────────────────────────
    /// Dismiss the help overlay.
    #[serde(rename = "help:dismiss")]
    HelpDismiss,

    // ── Attachment navigation ───────────────────────────────────────────
    /// Navigate to next attachment.
    #[serde(rename = "attachments:next")]
    AttachmentsNext,
    /// Navigate to previous attachment.
    #[serde(rename = "attachments:previous")]
    AttachmentsPrevious,
    /// Remove the selected attachment.
    #[serde(rename = "attachments:remove")]
    AttachmentsRemove,
    /// Exit attachment navigation.
    #[serde(rename = "attachments:exit")]
    AttachmentsExit,

    // ── Footer indicator actions ────────────────────────────────────────
    /// Navigate up in footer.
    #[serde(rename = "footer:up")]
    FooterUp,
    /// Navigate down in footer.
    #[serde(rename = "footer:down")]
    FooterDown,
    /// Navigate to next footer indicator.
    #[serde(rename = "footer:next")]
    FooterNext,
    /// Navigate to previous footer indicator.
    #[serde(rename = "footer:previous")]
    FooterPrevious,
    /// Open the selected footer item.
    #[serde(rename = "footer:openSelected")]
    FooterOpenSelected,
    /// Clear footer selection.
    #[serde(rename = "footer:clearSelection")]
    FooterClearSelection,
    /// Close footer.
    #[serde(rename = "footer:close")]
    FooterClose,

    // ── Message selector (rewind) actions ───────────────────────────────
    /// Move selection up in message selector.
    #[serde(rename = "messageSelector:up")]
    MessageSelectorUp,
    /// Move selection down in message selector.
    #[serde(rename = "messageSelector:down")]
    MessageSelectorDown,
    /// Jump to top of message selector.
    #[serde(rename = "messageSelector:top")]
    MessageSelectorTop,
    /// Jump to bottom of message selector.
    #[serde(rename = "messageSelector:bottom")]
    MessageSelectorBottom,
    /// Confirm selection in message selector.
    #[serde(rename = "messageSelector:select")]
    MessageSelectorSelect,

    // ── Diff dialog actions ─────────────────────────────────────────────
    /// Dismiss the diff dialog.
    #[serde(rename = "diff:dismiss")]
    DiffDismiss,
    /// Navigate to previous source in diff.
    #[serde(rename = "diff:previousSource")]
    DiffPreviousSource,
    /// Navigate to next source in diff.
    #[serde(rename = "diff:nextSource")]
    DiffNextSource,
    /// Go back in diff dialog.
    #[serde(rename = "diff:back")]
    DiffBack,
    /// View diff details.
    #[serde(rename = "diff:viewDetails")]
    DiffViewDetails,
    /// Navigate to previous file in diff.
    #[serde(rename = "diff:previousFile")]
    DiffPreviousFile,
    /// Navigate to next file in diff.
    #[serde(rename = "diff:nextFile")]
    DiffNextFile,

    // ── Model picker actions ────────────────────────────────────────────
    /// Decrease effort level in model picker.
    #[serde(rename = "modelPicker:decreaseEffort")]
    ModelPickerDecreaseEffort,
    /// Increase effort level in model picker.
    #[serde(rename = "modelPicker:increaseEffort")]
    ModelPickerIncreaseEffort,

    // ── Select component actions ────────────────────────────────────────
    /// Navigate to next item in select list.
    #[serde(rename = "select:next")]
    SelectNext,
    /// Navigate to previous item in select list.
    #[serde(rename = "select:previous")]
    SelectPrevious,
    /// Accept the current selection.
    #[serde(rename = "select:accept")]
    SelectAccept,
    /// Cancel the current selection.
    #[serde(rename = "select:cancel")]
    SelectCancel,

    // ── Plugin dialog actions ───────────────────────────────────────────
    /// Toggle a plugin on/off.
    #[serde(rename = "plugin:toggle")]
    PluginToggle,
    /// Install a plugin.
    #[serde(rename = "plugin:install")]
    PluginInstall,

    // ── Permission dialog actions ───────────────────────────────────────
    /// Toggle debug info in permission dialogs.
    #[serde(rename = "permission:toggleDebug")]
    PermissionToggleDebug,

    // ── Settings config panel actions ───────────────────────────────────
    /// Enter search mode in settings.
    #[serde(rename = "settings:search")]
    SettingsSearch,
    /// Retry loading usage data in settings.
    #[serde(rename = "settings:retry")]
    SettingsRetry,
    /// Save and close settings panel.
    #[serde(rename = "settings:close")]
    SettingsClose,

    // ── Scroll actions ──────────────────────────────────────────────────
    /// Scroll up one page.
    #[serde(rename = "scroll:pageUp")]
    ScrollPageUp,
    /// Scroll down one page.
    #[serde(rename = "scroll:pageDown")]
    ScrollPageDown,
    /// Scroll up one line (mouse wheel).
    #[serde(rename = "scroll:lineUp")]
    ScrollLineUp,
    /// Scroll down one line (mouse wheel).
    #[serde(rename = "scroll:lineDown")]
    ScrollLineDown,
    /// Scroll to the top.
    #[serde(rename = "scroll:top")]
    ScrollTop,
    /// Scroll to the bottom.
    #[serde(rename = "scroll:bottom")]
    ScrollBottom,

    // ── Selection actions ───────────────────────────────────────────────
    /// Copy selection to clipboard.
    #[serde(rename = "selection:copy")]
    SelectionCopy,

    // ── Voice actions ───────────────────────────────────────────────────
    /// Push-to-talk voice activation.
    #[serde(rename = "voice:pushToTalk")]
    VoicePushToTalk,

    // ── Command bindings ────────────────────────────────────────────────
    /// A dynamic command binding (e.g., `command:help`, `command:compact`).
    /// Executes the slash command as if typed.
    #[serde(untagged)]
    Command(String),
}

impl fmt::Display for KeybindingAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(cmd) => write!(f, "{}", cmd),
            other => {
                // Use serde serialization to get the rename string
                let json = serde_json::to_string(other).unwrap_or_default();
                // Remove surrounding quotes
                let s = json.trim_matches('"');
                write!(f, "{}", s)
            }
        }
    }
}

impl KeybindingAction {
    /// Parse an action string. Returns `None` for null (unbind).
    /// Returns `Some(Command(s))` for `command:*` patterns.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        // Try deserializing from the serde rename
        if let Ok(action) = serde_json::from_value::<KeybindingAction>(
            serde_json::Value::String(s.to_string()),
        ) {
            // If it round-trips to a Command variant but isn't a command: prefix,
            // that means serde used the untagged fallback
            if matches!(&action, Self::Command(c) if !c.starts_with("command:")) {
                return None;
            }
            return Some(action);
        }
        None
    }

    /// All known non-command action variants.
    pub const ALL: &'static [KeybindingAction] = &[
        Self::AppInterrupt,
        Self::AppExit,
        Self::AppToggleTodos,
        Self::AppToggleTranscript,
        Self::AppToggleBrief,
        Self::AppToggleTeammatePreview,
        Self::AppToggleTerminal,
        Self::AppRedraw,
        Self::AppGlobalSearch,
        Self::AppQuickOpen,
        Self::HistorySearch,
        Self::HistoryPrevious,
        Self::HistoryNext,
        Self::ChatCancel,
        Self::ChatKillAgents,
        Self::ChatCycleMode,
        Self::ChatModelPicker,
        Self::ChatFastMode,
        Self::ChatThinkingToggle,
        Self::ChatSubmit,
        Self::ChatNewLine,
        Self::ChatUndo,
        Self::ChatExternalEditor,
        Self::ChatStash,
        Self::ChatImagePaste,
        Self::ChatMessageActions,
        Self::AutocompleteAccept,
        Self::AutocompleteDismiss,
        Self::AutocompletePrevious,
        Self::AutocompleteNext,
        Self::ConfirmYes,
        Self::ConfirmNo,
        Self::ConfirmPrevious,
        Self::ConfirmNext,
        Self::ConfirmNextField,
        Self::ConfirmPreviousField,
        Self::ConfirmCycleMode,
        Self::ConfirmToggle,
        Self::ConfirmToggleExplanation,
        Self::TabsNext,
        Self::TabsPrevious,
        Self::TranscriptToggleShowAll,
        Self::TranscriptExit,
        Self::HistorySearchNext,
        Self::HistorySearchAccept,
        Self::HistorySearchCancel,
        Self::HistorySearchExecute,
        Self::TaskBackground,
        Self::ThemeToggleSyntaxHighlighting,
        Self::HelpDismiss,
        Self::AttachmentsNext,
        Self::AttachmentsPrevious,
        Self::AttachmentsRemove,
        Self::AttachmentsExit,
        Self::FooterUp,
        Self::FooterDown,
        Self::FooterNext,
        Self::FooterPrevious,
        Self::FooterOpenSelected,
        Self::FooterClearSelection,
        Self::FooterClose,
        Self::MessageSelectorUp,
        Self::MessageSelectorDown,
        Self::MessageSelectorTop,
        Self::MessageSelectorBottom,
        Self::MessageSelectorSelect,
        Self::DiffDismiss,
        Self::DiffPreviousSource,
        Self::DiffNextSource,
        Self::DiffBack,
        Self::DiffViewDetails,
        Self::DiffPreviousFile,
        Self::DiffNextFile,
        Self::ModelPickerDecreaseEffort,
        Self::ModelPickerIncreaseEffort,
        Self::SelectNext,
        Self::SelectPrevious,
        Self::SelectAccept,
        Self::SelectCancel,
        Self::PluginToggle,
        Self::PluginInstall,
        Self::PermissionToggleDebug,
        Self::SettingsSearch,
        Self::SettingsRetry,
        Self::SettingsClose,
        Self::ScrollPageUp,
        Self::ScrollPageDown,
        Self::ScrollLineUp,
        Self::ScrollLineDown,
        Self::ScrollTop,
        Self::ScrollBottom,
        Self::SelectionCopy,
        Self::VoicePushToTalk,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_serde_roundtrip() {
        let action = KeybindingAction::ChatSubmit;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"chat:submit\"");
        let back: KeybindingAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, action);
    }

    #[test]
    fn test_action_display() {
        assert_eq!(KeybindingAction::AppInterrupt.to_string(), "app:interrupt");
        assert_eq!(KeybindingAction::ChatSubmit.to_string(), "chat:submit");
        assert_eq!(
            KeybindingAction::AutocompleteAccept.to_string(),
            "autocomplete:accept"
        );
    }

    #[test]
    fn test_from_str_opt() {
        assert_eq!(
            KeybindingAction::from_str_opt("app:interrupt"),
            Some(KeybindingAction::AppInterrupt)
        );
        assert_eq!(
            KeybindingAction::from_str_opt("chat:submit"),
            Some(KeybindingAction::ChatSubmit)
        );
        assert_eq!(
            KeybindingAction::from_str_opt("command:help"),
            Some(KeybindingAction::Command("command:help".to_string()))
        );
        assert_eq!(KeybindingAction::from_str_opt("nonexistent:action"), None);
    }

    #[test]
    fn test_command_variant() {
        let cmd = KeybindingAction::Command("command:compact".to_string());
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, "\"command:compact\"");
        let back: KeybindingAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn test_all_actions_count() {
        // 93 non-command actions
        assert_eq!(KeybindingAction::ALL.len(), 93);
    }

    #[test]
    fn test_all_actions_unique() {
        let mut seen = std::collections::HashSet::new();
        for action in KeybindingAction::ALL {
            assert!(
                seen.insert(action.to_string()),
                "duplicate action: {}",
                action
            );
        }
    }
}
