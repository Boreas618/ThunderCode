//! Keybinding contexts define where bindings are active.
//!
//! Contexts form a priority hierarchy: more specific contexts (e.g., `Autocomplete`)
//! take precedence over broader ones (e.g., `Chat`, `Global`). When resolving a key
//! event, the resolver checks the most specific active context first.

use serde::{Deserialize, Serialize};
use std::fmt;

/// All valid UI contexts where keybindings can be applied.
///
/// Ported from `KEYBINDING_CONTEXTS` in `schema.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeybindingContext {
    /// Active everywhere, regardless of focus.
    Global,
    /// When the chat input is focused.
    Chat,
    /// When autocomplete menu is visible.
    Autocomplete,
    /// When a confirmation/permission dialog is shown.
    Confirmation,
    /// When the help overlay is open.
    Help,
    /// When viewing the transcript.
    Transcript,
    /// When searching command history (ctrl+r).
    HistorySearch,
    /// When a task/agent is running in the foreground.
    Task,
    /// When the theme picker is open.
    ThemePicker,
    /// When the settings menu is open.
    Settings,
    /// When tab navigation is active.
    Tabs,
    /// When navigating image attachments in a select dialog.
    Attachments,
    /// When footer indicators are focused.
    Footer,
    /// When the message selector (rewind) is open.
    MessageSelector,
    /// When the diff dialog is open.
    DiffDialog,
    /// When the model picker is open.
    ModelPicker,
    /// When a select/list component is focused.
    Select,
    /// When the plugin dialog is open.
    Plugin,
    /// Scroll context for page-level scrolling and copy.
    Scroll,
}

impl fmt::Display for KeybindingContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Global => "Global",
            Self::Chat => "Chat",
            Self::Autocomplete => "Autocomplete",
            Self::Confirmation => "Confirmation",
            Self::Help => "Help",
            Self::Transcript => "Transcript",
            Self::HistorySearch => "HistorySearch",
            Self::Task => "Task",
            Self::ThemePicker => "ThemePicker",
            Self::Settings => "Settings",
            Self::Tabs => "Tabs",
            Self::Attachments => "Attachments",
            Self::Footer => "Footer",
            Self::MessageSelector => "MessageSelector",
            Self::DiffDialog => "DiffDialog",
            Self::ModelPicker => "ModelPicker",
            Self::Select => "Select",
            Self::Plugin => "Plugin",
            Self::Scroll => "Scroll",
        };
        write!(f, "{}", name)
    }
}

impl KeybindingContext {
    /// All known context variants.
    pub const ALL: &'static [KeybindingContext] = &[
        Self::Global,
        Self::Chat,
        Self::Autocomplete,
        Self::Confirmation,
        Self::Help,
        Self::Transcript,
        Self::HistorySearch,
        Self::Task,
        Self::ThemePicker,
        Self::Settings,
        Self::Tabs,
        Self::Attachments,
        Self::Footer,
        Self::MessageSelector,
        Self::DiffDialog,
        Self::ModelPicker,
        Self::Select,
        Self::Plugin,
        Self::Scroll,
    ];

    /// Human-readable description of the context.
    ///
    /// Ported from `KEYBINDING_CONTEXT_DESCRIPTIONS` in `schema.ts`.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Global => "Active everywhere, regardless of focus",
            Self::Chat => "When the chat input is focused",
            Self::Autocomplete => "When autocomplete menu is visible",
            Self::Confirmation => "When a confirmation/permission dialog is shown",
            Self::Help => "When the help overlay is open",
            Self::Transcript => "When viewing the transcript",
            Self::HistorySearch => "When searching command history (ctrl+r)",
            Self::Task => "When a task/agent is running in the foreground",
            Self::ThemePicker => "When the theme picker is open",
            Self::Settings => "When the settings menu is open",
            Self::Tabs => "When tab navigation is active",
            Self::Attachments => "When navigating image attachments in a select dialog",
            Self::Footer => "When footer indicators are focused",
            Self::MessageSelector => "When the message selector (rewind) is open",
            Self::DiffDialog => "When the diff dialog is open",
            Self::ModelPicker => "When the model picker is open",
            Self::Select => "When a select/list component is focused",
            Self::Plugin => "When the plugin dialog is open",
            Self::Scroll => "When page-level scroll or copy is active",
        }
    }

    /// Parse a context name string (case-sensitive).
    pub fn from_str_exact(s: &str) -> Option<Self> {
        match s {
            "Global" => Some(Self::Global),
            "Chat" => Some(Self::Chat),
            "Autocomplete" => Some(Self::Autocomplete),
            "Confirmation" => Some(Self::Confirmation),
            "Help" => Some(Self::Help),
            "Transcript" => Some(Self::Transcript),
            "HistorySearch" => Some(Self::HistorySearch),
            "Task" => Some(Self::Task),
            "ThemePicker" => Some(Self::ThemePicker),
            "Settings" => Some(Self::Settings),
            "Tabs" => Some(Self::Tabs),
            "Attachments" => Some(Self::Attachments),
            "Footer" => Some(Self::Footer),
            "MessageSelector" => Some(Self::MessageSelector),
            "DiffDialog" => Some(Self::DiffDialog),
            "ModelPicker" => Some(Self::ModelPicker),
            "Select" => Some(Self::Select),
            "Plugin" => Some(Self::Plugin),
            "Scroll" => Some(Self::Scroll),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_display() {
        assert_eq!(KeybindingContext::Global.to_string(), "Global");
        assert_eq!(KeybindingContext::Chat.to_string(), "Chat");
        assert_eq!(
            KeybindingContext::HistorySearch.to_string(),
            "HistorySearch"
        );
    }

    #[test]
    fn test_from_str_exact() {
        assert_eq!(
            KeybindingContext::from_str_exact("Global"),
            Some(KeybindingContext::Global)
        );
        assert_eq!(
            KeybindingContext::from_str_exact("HistorySearch"),
            Some(KeybindingContext::HistorySearch)
        );
        assert_eq!(KeybindingContext::from_str_exact("global"), None);
        assert_eq!(KeybindingContext::from_str_exact("unknown"), None);
    }

    #[test]
    fn test_all_contexts_listed() {
        assert_eq!(KeybindingContext::ALL.len(), 19);
    }

    #[test]
    fn test_context_serde_roundtrip() {
        let ctx = KeybindingContext::Autocomplete;
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, "\"Autocomplete\"");
        let back: KeybindingContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ctx);
    }
}
