//! Key binding definitions and default binding map.
//!
//! Ported from `defaultBindings.ts`. This module defines the `KeyCombo` and
//! `KeyBinding` types plus the full default binding table.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::keybindings::actions::KeybindingAction;
use crate::keybindings::context::KeybindingContext;

// ─── Key combo ──────────────────────────────────────────────────────────────

/// A single keystroke with optional modifiers.
///
/// Corresponds to `ParsedKeystroke` in `parser.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    /// The base key name (e.g., `"enter"`, `"tab"`, `"a"`, `"f1"`, `" "`).
    pub key: String,
    /// Ctrl modifier.
    pub ctrl: bool,
    /// Shift modifier.
    pub shift: bool,
    /// Alt modifier (Option on macOS; equivalent to meta in terminals).
    pub alt: bool,
    /// Meta modifier (treated as alias for alt in legacy terminals).
    pub meta: bool,
    /// Super modifier (Cmd on macOS, Win key). Only arrives via kitty protocol.
    #[serde(rename = "super")]
    pub super_key: bool,
}

impl Default for KeyCombo {
    fn default() -> Self {
        Self {
            key: String::new(),
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
            super_key: false,
        }
    }
}

impl KeyCombo {
    /// Create a simple key with no modifiers.
    pub fn key(name: &str) -> Self {
        Self {
            key: name.to_string(),
            ..Default::default()
        }
    }

    /// Create a Ctrl+key combo.
    pub fn ctrl(name: &str) -> Self {
        Self {
            key: name.to_string(),
            ctrl: true,
            ..Default::default()
        }
    }

    /// Create a Shift+key combo.
    pub fn shift(name: &str) -> Self {
        Self {
            key: name.to_string(),
            shift: true,
            ..Default::default()
        }
    }

    /// Create a Meta/Alt+key combo.
    pub fn meta(name: &str) -> Self {
        Self {
            key: name.to_string(),
            alt: true,
            meta: true,
            ..Default::default()
        }
    }

    /// Create a Ctrl+Shift+key combo.
    pub fn ctrl_shift(name: &str) -> Self {
        Self {
            key: name.to_string(),
            ctrl: true,
            shift: true,
            ..Default::default()
        }
    }

    /// Create a Cmd/Super+key combo.
    pub fn cmd(name: &str) -> Self {
        Self {
            key: name.to_string(),
            super_key: true,
            ..Default::default()
        }
    }

    /// Create a Cmd+Shift combo.
    pub fn cmd_shift(name: &str) -> Self {
        Self {
            key: name.to_string(),
            super_key: true,
            shift: true,
            ..Default::default()
        }
    }

    /// Check if two `KeyCombo`s match for resolution purposes.
    ///
    /// Alt and meta are collapsed into one logical modifier (legacy terminals
    /// cannot distinguish them), matching the `keystrokesEqual` logic from
    /// `resolver.ts`. Super (cmd/win) is distinct.
    pub fn matches(&self, other: &KeyCombo) -> bool {
        self.key == other.key
            && self.ctrl == other.ctrl
            && self.shift == other.shift
            && (self.alt || self.meta) == (other.alt || other.meta)
            && self.super_key == other.super_key
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.shift {
            parts.push("shift");
        }
        if self.meta && !self.alt {
            parts.push("meta");
        }
        if self.super_key {
            parts.push("cmd");
        }
        // Use readable display names for special keys.
        let display_key = match self.key.as_str() {
            "escape" => "Esc",
            " " => "Space",
            "tab" => "tab",
            "enter" => "Enter",
            "backspace" => "Backspace",
            "delete" => "Delete",
            "up" => "Up",
            "down" => "Down",
            "left" => "Left",
            "right" => "Right",
            "pageup" => "PageUp",
            "pagedown" => "PageDown",
            "home" => "Home",
            "end" => "End",
            other => other,
        };
        parts.push(display_key);
        write!(f, "{}", parts.join("+"))
    }
}

// ─── Parsing ────────────────────────────────────────────────────────────────

/// Parse a keystroke string like `"ctrl+shift+k"` into a `KeyCombo`.
///
/// Ported from `parseKeystroke` in `parser.ts`. Supports modifier aliases:
/// `ctrl`/`control`, `alt`/`opt`/`option`/`meta`, `cmd`/`command`/`super`/`win`.
pub fn parse_keystroke(input: &str) -> KeyCombo {
    let parts: Vec<&str> = input.split('+').collect();
    let mut combo = KeyCombo::default();

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => combo.ctrl = true,
            "alt" | "opt" | "option" => combo.alt = true,
            "shift" => combo.shift = true,
            "meta" => combo.meta = true,
            "cmd" | "command" | "super" | "win" => combo.super_key = true,
            "esc" => combo.key = "escape".to_string(),
            "return" => combo.key = "enter".to_string(),
            "space" => combo.key = " ".to_string(),
            // Unicode arrow aliases
            "\u{2191}" => combo.key = "up".to_string(),
            "\u{2193}" => combo.key = "down".to_string(),
            "\u{2190}" => combo.key = "left".to_string(),
            "\u{2192}" => combo.key = "right".to_string(),
            other => combo.key = other.to_lowercase(),
        }
    }

    combo
}

/// Parse a chord string like `"ctrl+k ctrl+s"` into a list of `KeyCombo`s.
///
/// A lone space character (`" "`) is the space key binding, not a separator.
/// Ported from `parseChord` in `parser.ts`.
pub fn parse_chord(input: &str) -> Vec<KeyCombo> {
    // A lone space character IS the space key binding
    if input == " " {
        return vec![parse_keystroke("space")];
    }
    input
        .split_whitespace()
        .map(|s| parse_keystroke(s))
        .collect()
}

/// Convert a chord sequence to its canonical display string.
pub fn chord_to_string(chord: &[KeyCombo]) -> String {
    chord
        .iter()
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── Binding ────────────────────────────────────────────────────────────────

/// A single keybinding that maps a key combo (or chord) to an action in a context.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyBinding {
    /// The chord sequence (one element for simple bindings, multiple for chords).
    pub chord: Vec<KeyCombo>,
    /// The action to trigger, or `None` for an explicit unbind.
    pub action: Option<KeybindingAction>,
    /// The context in which this binding is active.
    pub context: KeybindingContext,
}

impl KeyBinding {
    /// Create a single-key binding.
    fn single(key: KeyCombo, action: KeybindingAction, context: KeybindingContext) -> Self {
        Self {
            chord: vec![key],
            action: Some(action),
            context,
        }
    }

    /// Create a chord binding.
    fn chord(
        keys: Vec<KeyCombo>,
        action: KeybindingAction,
        context: KeybindingContext,
    ) -> Self {
        Self {
            chord: keys,
            action: Some(action),
            context,
        }
    }
}

// ─── KeybindingBlock (for JSON config) ──────────────────────────────────────

/// A block of keybindings for a specific context, as stored in `keybindings.json`.
///
/// Maps keystroke pattern strings to action strings (or `null` to unbind).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingBlock {
    /// The UI context.
    pub context: String,
    /// Map of keystroke patterns to actions.
    pub bindings: std::collections::HashMap<String, Option<String>>,
}

/// Parse keybinding blocks (from JSON config) into a flat list of `KeyBinding`s.
///
/// Ported from `parseBindings` in `parser.ts`.
pub fn parse_blocks(blocks: &[KeybindingBlock]) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for block in blocks {
        let context = match KeybindingContext::from_str_exact(&block.context) {
            Some(c) => c,
            None => continue,
        };
        for (key_str, action_str) in &block.bindings {
            let chord = parse_chord(key_str);
            let action = action_str
                .as_ref()
                .and_then(|s| KeybindingAction::from_str_opt(s));
            bindings.push(KeyBinding {
                chord,
                action,
                context,
            });
        }
    }
    bindings
}

// ─── Default bindings ───────────────────────────────────────────────────────

/// Build the complete default binding table.
///
/// This is an exact port of `DEFAULT_BINDINGS` from `defaultBindings.ts`.
/// Feature-gated bindings (KAIROS, QUICK_SEARCH, etc.) are omitted since
/// feature flags are not yet available in the Rust port; they can be
/// conditionally appended by the caller.
pub fn get_default_bindings() -> Vec<KeyBinding> {
    use KeybindingAction::*;
    use KeybindingContext::*;

    let b = KeyBinding::single;

    let mut v: Vec<KeyBinding> = Vec::with_capacity(128);

    // ── Global ──────────────────────────────────────────────────────────
    v.push(b(KeyCombo::ctrl("c"), AppInterrupt, Global));
    v.push(b(KeyCombo::ctrl("d"), AppExit, Global));
    v.push(b(KeyCombo::ctrl("l"), AppRedraw, Global));
    v.push(b(KeyCombo::ctrl("t"), AppToggleTodos, Global));
    v.push(b(KeyCombo::ctrl("o"), AppToggleTranscript, Global));
    v.push(b(
        KeyCombo::ctrl_shift("o"),
        AppToggleTeammatePreview,
        Global,
    ));
    v.push(b(KeyCombo::ctrl("r"), KeybindingAction::HistorySearch, Global));

    // ── Chat ────────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("escape"), ChatCancel, Chat));
    // ctrl+x ctrl+k chord
    v.push(KeyBinding::chord(
        vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
        ChatKillAgents,
        Chat,
    ));
    v.push(b(KeyCombo::shift("tab"), ChatCycleMode, Chat));
    v.push(b(KeyCombo::meta("p"), ChatModelPicker, Chat));
    v.push(b(KeyCombo::meta("o"), ChatFastMode, Chat));
    v.push(b(KeyCombo::meta("t"), ChatThinkingToggle, Chat));
    v.push(b(KeyCombo::key("enter"), ChatSubmit, Chat));
    v.push(b(KeyCombo::key("up"), HistoryPrevious, Chat));
    v.push(b(KeyCombo::key("down"), HistoryNext, Chat));
    // Undo: two bindings for different terminal behaviors
    v.push(b(KeyCombo::ctrl("_"), ChatUndo, Chat));
    v.push(b(KeyCombo::ctrl_shift("-"), ChatUndo, Chat));
    // External editor: ctrl+x ctrl+e chord and ctrl+g direct
    v.push(KeyBinding::chord(
        vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("e")],
        ChatExternalEditor,
        Chat,
    ));
    v.push(b(KeyCombo::ctrl("g"), ChatExternalEditor, Chat));
    v.push(b(KeyCombo::ctrl("s"), ChatStash, Chat));
    // Image paste (non-Windows default: ctrl+v)
    v.push(b(KeyCombo::ctrl("v"), ChatImagePaste, Chat));

    // ── Autocomplete ────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("tab"), AutocompleteAccept, Autocomplete));
    v.push(b(
        KeyCombo::key("escape"),
        AutocompleteDismiss,
        Autocomplete,
    ));
    v.push(b(KeyCombo::key("up"), AutocompletePrevious, Autocomplete));
    v.push(b(KeyCombo::key("down"), AutocompleteNext, Autocomplete));

    // ── Settings ────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("escape"), ConfirmNo, Settings));
    v.push(b(KeyCombo::key("up"), SelectPrevious, Settings));
    v.push(b(KeyCombo::key("down"), SelectNext, Settings));
    v.push(b(KeyCombo::key("k"), SelectPrevious, Settings));
    v.push(b(KeyCombo::key("j"), SelectNext, Settings));
    v.push(b(KeyCombo::ctrl("p"), SelectPrevious, Settings));
    v.push(b(KeyCombo::ctrl("n"), SelectNext, Settings));
    v.push(b(KeyCombo::key(" "), SelectAccept, Settings));
    v.push(b(KeyCombo::key("enter"), SettingsClose, Settings));
    v.push(b(KeyCombo::key("/"), SettingsSearch, Settings));
    v.push(b(KeyCombo::key("r"), SettingsRetry, Settings));

    // ── Confirmation ────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("y"), ConfirmYes, Confirmation));
    v.push(b(KeyCombo::key("n"), ConfirmNo, Confirmation));
    v.push(b(KeyCombo::key("enter"), ConfirmYes, Confirmation));
    v.push(b(KeyCombo::key("escape"), ConfirmNo, Confirmation));
    v.push(b(KeyCombo::key("up"), ConfirmPrevious, Confirmation));
    v.push(b(KeyCombo::key("down"), ConfirmNext, Confirmation));
    v.push(b(KeyCombo::key("tab"), ConfirmNextField, Confirmation));
    v.push(b(KeyCombo::key(" "), ConfirmToggle, Confirmation));
    v.push(b(KeyCombo::shift("tab"), ConfirmCycleMode, Confirmation));
    v.push(b(
        KeyCombo::ctrl("e"),
        ConfirmToggleExplanation,
        Confirmation,
    ));
    v.push(b(
        KeyCombo::ctrl("d"),
        PermissionToggleDebug,
        Confirmation,
    ));

    // ── Tabs ────────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("tab"), TabsNext, Tabs));
    v.push(b(KeyCombo::shift("tab"), TabsPrevious, Tabs));
    v.push(b(KeyCombo::key("right"), TabsNext, Tabs));
    v.push(b(KeyCombo::key("left"), TabsPrevious, Tabs));

    // ── Transcript ──────────────────────────────────────────────────────
    v.push(b(
        KeyCombo::ctrl("e"),
        TranscriptToggleShowAll,
        Transcript,
    ));
    v.push(b(KeyCombo::ctrl("c"), TranscriptExit, Transcript));
    v.push(b(KeyCombo::key("escape"), TranscriptExit, Transcript));
    v.push(b(KeyCombo::key("q"), TranscriptExit, Transcript));

    // ── HistorySearch ───────────────────────────────────────────────────
    // Use fully-qualified context to disambiguate from KeybindingAction::HistorySearch
    let hs_ctx = KeybindingContext::HistorySearch;
    v.push(b(KeyCombo::ctrl("r"), HistorySearchNext, hs_ctx));
    v.push(b(KeyCombo::key("escape"), HistorySearchAccept, hs_ctx));
    v.push(b(KeyCombo::key("tab"), HistorySearchAccept, hs_ctx));
    v.push(b(KeyCombo::ctrl("c"), HistorySearchCancel, hs_ctx));
    v.push(b(KeyCombo::key("enter"), HistorySearchExecute, hs_ctx));

    // ── Task ────────────────────────────────────────────────────────────
    v.push(b(KeyCombo::ctrl("b"), TaskBackground, Task));

    // ── ThemePicker ─────────────────────────────────────────────────────
    v.push(b(
        KeyCombo::ctrl("t"),
        ThemeToggleSyntaxHighlighting,
        ThemePicker,
    ));

    // ── Scroll ──────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("pageup"), ScrollPageUp, Scroll));
    v.push(b(KeyCombo::key("pagedown"), ScrollPageDown, Scroll));
    v.push(b(KeyCombo::key("wheelup"), ScrollLineUp, Scroll));
    v.push(b(KeyCombo::key("wheeldown"), ScrollLineDown, Scroll));
    v.push(b(KeyCombo::ctrl("home"), ScrollTop, Scroll));
    v.push(b(KeyCombo::ctrl("end"), ScrollBottom, Scroll));
    v.push(b(KeyCombo::ctrl_shift("c"), SelectionCopy, Scroll));
    v.push(b(KeyCombo::cmd("c"), SelectionCopy, Scroll));

    // ── Help ────────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("escape"), HelpDismiss, Help));

    // ── Attachments ─────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("right"), AttachmentsNext, Attachments));
    v.push(b(KeyCombo::key("left"), AttachmentsPrevious, Attachments));
    v.push(b(
        KeyCombo::key("backspace"),
        AttachmentsRemove,
        Attachments,
    ));
    v.push(b(KeyCombo::key("delete"), AttachmentsRemove, Attachments));
    v.push(b(KeyCombo::key("down"), AttachmentsExit, Attachments));
    v.push(b(KeyCombo::key("escape"), AttachmentsExit, Attachments));

    // ── Footer ──────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("up"), FooterUp, Footer));
    v.push(b(KeyCombo::ctrl("p"), FooterUp, Footer));
    v.push(b(KeyCombo::key("down"), FooterDown, Footer));
    v.push(b(KeyCombo::ctrl("n"), FooterDown, Footer));
    v.push(b(KeyCombo::key("right"), FooterNext, Footer));
    v.push(b(KeyCombo::key("left"), FooterPrevious, Footer));
    v.push(b(KeyCombo::key("enter"), FooterOpenSelected, Footer));
    v.push(b(
        KeyCombo::key("escape"),
        FooterClearSelection,
        Footer,
    ));

    // ── MessageSelector ─────────────────────────────────────────────────
    v.push(b(KeyCombo::key("up"), MessageSelectorUp, MessageSelector));
    v.push(b(
        KeyCombo::key("down"),
        MessageSelectorDown,
        MessageSelector,
    ));
    v.push(b(KeyCombo::key("k"), MessageSelectorUp, MessageSelector));
    v.push(b(
        KeyCombo::key("j"),
        MessageSelectorDown,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::ctrl("p"),
        MessageSelectorUp,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::ctrl("n"),
        MessageSelectorDown,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::ctrl("up"),
        MessageSelectorTop,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::shift("up"),
        MessageSelectorTop,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::meta("up"),
        MessageSelectorTop,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::shift("k"),
        MessageSelectorTop,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::ctrl("down"),
        MessageSelectorBottom,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::shift("down"),
        MessageSelectorBottom,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::meta("down"),
        MessageSelectorBottom,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::shift("j"),
        MessageSelectorBottom,
        MessageSelector,
    ));
    v.push(b(
        KeyCombo::key("enter"),
        MessageSelectorSelect,
        MessageSelector,
    ));

    // ── DiffDialog ──────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("escape"), DiffDismiss, DiffDialog));
    v.push(b(KeyCombo::key("left"), DiffPreviousSource, DiffDialog));
    v.push(b(KeyCombo::key("right"), DiffNextSource, DiffDialog));
    v.push(b(KeyCombo::key("up"), DiffPreviousFile, DiffDialog));
    v.push(b(KeyCombo::key("down"), DiffNextFile, DiffDialog));
    v.push(b(KeyCombo::key("enter"), DiffViewDetails, DiffDialog));

    // ── ModelPicker ─────────────────────────────────────────────────────
    v.push(b(
        KeyCombo::key("left"),
        ModelPickerDecreaseEffort,
        ModelPicker,
    ));
    v.push(b(
        KeyCombo::key("right"),
        ModelPickerIncreaseEffort,
        ModelPicker,
    ));

    // ── Select ──────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key("up"), SelectPrevious, Select));
    v.push(b(KeyCombo::key("down"), SelectNext, Select));
    v.push(b(KeyCombo::key("j"), SelectNext, Select));
    v.push(b(KeyCombo::key("k"), SelectPrevious, Select));
    v.push(b(KeyCombo::ctrl("n"), SelectNext, Select));
    v.push(b(KeyCombo::ctrl("p"), SelectPrevious, Select));
    v.push(b(KeyCombo::key("enter"), SelectAccept, Select));
    v.push(b(KeyCombo::key("escape"), SelectCancel, Select));

    // ── Plugin ──────────────────────────────────────────────────────────
    v.push(b(KeyCombo::key(" "), PluginToggle, Plugin));
    v.push(b(KeyCombo::key("i"), PluginInstall, Plugin));

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_keystroke_simple() {
        let k = parse_keystroke("enter");
        assert_eq!(k.key, "enter");
        assert!(!k.ctrl);
        assert!(!k.shift);
    }

    #[test]
    fn test_parse_keystroke_ctrl() {
        let k = parse_keystroke("ctrl+c");
        assert_eq!(k.key, "c");
        assert!(k.ctrl);
        assert!(!k.shift);
    }

    #[test]
    fn test_parse_keystroke_ctrl_shift() {
        let k = parse_keystroke("ctrl+shift+o");
        assert_eq!(k.key, "o");
        assert!(k.ctrl);
        assert!(k.shift);
    }

    #[test]
    fn test_parse_keystroke_meta() {
        let k = parse_keystroke("meta+p");
        assert_eq!(k.key, "p");
        assert!(k.meta);
        assert!(!k.ctrl);
    }

    #[test]
    fn test_parse_keystroke_aliases() {
        let k = parse_keystroke("control+option+k");
        assert_eq!(k.key, "k");
        assert!(k.ctrl);
        assert!(k.alt);

        let k2 = parse_keystroke("cmd+c");
        assert_eq!(k2.key, "c");
        assert!(k2.super_key);
    }

    #[test]
    fn test_parse_chord_single() {
        let chord = parse_chord("ctrl+c");
        assert_eq!(chord.len(), 1);
        assert_eq!(chord[0].key, "c");
        assert!(chord[0].ctrl);
    }

    #[test]
    fn test_parse_chord_multi() {
        let chord = parse_chord("ctrl+x ctrl+k");
        assert_eq!(chord.len(), 2);
        assert_eq!(chord[0].key, "x");
        assert!(chord[0].ctrl);
        assert_eq!(chord[1].key, "k");
        assert!(chord[1].ctrl);
    }

    #[test]
    fn test_parse_chord_space_key() {
        let chord = parse_chord(" ");
        assert_eq!(chord.len(), 1);
        assert_eq!(chord[0].key, " ");
    }

    #[test]
    fn test_key_combo_display() {
        assert_eq!(KeyCombo::ctrl("c").to_string(), "ctrl+c");
        assert_eq!(KeyCombo::shift("tab").to_string(), "shift+tab");
        assert_eq!(KeyCombo::key("escape").to_string(), "Esc");
        assert_eq!(KeyCombo::key("enter").to_string(), "Enter");
        assert_eq!(KeyCombo::meta("p").to_string(), "alt+p");
        assert_eq!(KeyCombo::cmd("c").to_string(), "cmd+c");
    }

    #[test]
    fn test_key_combo_matches() {
        assert!(KeyCombo::ctrl("c").matches(&KeyCombo::ctrl("c")));
        assert!(!KeyCombo::ctrl("c").matches(&KeyCombo::ctrl("d")));

        // Alt and meta are equivalent
        let alt_k = KeyCombo {
            key: "k".into(),
            alt: true,
            ..Default::default()
        };
        let meta_k = KeyCombo {
            key: "k".into(),
            meta: true,
            ..Default::default()
        };
        assert!(alt_k.matches(&meta_k));
    }

    #[test]
    fn test_default_bindings_not_empty() {
        let bindings = get_default_bindings();
        assert!(bindings.len() > 100);
    }

    #[test]
    fn test_default_bindings_have_expected_entries() {
        let bindings = get_default_bindings();

        // ctrl+c -> AppInterrupt in Global
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Global
                && b.action == Some(KeybindingAction::AppInterrupt)
                && b.chord.len() == 1
                && b.chord[0].ctrl
                && b.chord[0].key == "c"
        });
        assert!(found, "missing ctrl+c -> AppInterrupt in Global");

        // enter -> ChatSubmit in Chat
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Chat
                && b.action == Some(KeybindingAction::ChatSubmit)
                && b.chord.len() == 1
                && b.chord[0].key == "enter"
        });
        assert!(found, "missing enter -> ChatSubmit in Chat");

        // tab -> AutocompleteAccept in Autocomplete
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Autocomplete
                && b.action == Some(KeybindingAction::AutocompleteAccept)
                && b.chord.len() == 1
                && b.chord[0].key == "tab"
        });
        assert!(found, "missing tab -> AutocompleteAccept in Autocomplete");

        // y -> ConfirmYes in Confirmation
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Confirmation
                && b.action == Some(KeybindingAction::ConfirmYes)
                && b.chord.len() == 1
                && b.chord[0].key == "y"
        });
        assert!(found, "missing y -> ConfirmYes in Confirmation");
    }

    #[test]
    fn test_default_bindings_have_chords() {
        let bindings = get_default_bindings();

        // ctrl+x ctrl+k -> ChatKillAgents
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Chat
                && b.action == Some(KeybindingAction::ChatKillAgents)
                && b.chord.len() == 2
                && b.chord[0].ctrl
                && b.chord[0].key == "x"
                && b.chord[1].ctrl
                && b.chord[1].key == "k"
        });
        assert!(found, "missing ctrl+x ctrl+k chord");

        // ctrl+x ctrl+e -> ChatExternalEditor
        let found = bindings.iter().any(|b| {
            b.context == KeybindingContext::Chat
                && b.action == Some(KeybindingAction::ChatExternalEditor)
                && b.chord.len() == 2
                && b.chord[0].ctrl
                && b.chord[0].key == "x"
                && b.chord[1].ctrl
                && b.chord[1].key == "e"
        });
        assert!(found, "missing ctrl+x ctrl+e chord");
    }
}
