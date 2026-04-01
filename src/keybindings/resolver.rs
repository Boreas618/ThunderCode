//! Keybinding resolution engine.
//!
//! Given a key event and the list of active contexts, the resolver finds the
//! matching action. User custom bindings override defaults (last-wins
//! semantics, same as the TypeScript resolver).
//!
//! Ported from `resolver.ts` and `loadUserBindings.ts`.

use std::collections::HashSet;
use std::path::Path;

use crate::keybindings::actions::KeybindingAction;
use crate::keybindings::bindings::{
    get_default_bindings, parse_blocks, KeyBinding, KeyCombo, KeybindingBlock,
};
use crate::keybindings::chord::{ChordMatchResult, ChordMatcher};
use crate::keybindings::context::KeybindingContext;

/// Result of resolving a key event.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveResult {
    /// A binding matched. Contains the resolved action.
    Match(KeybindingAction),
    /// A chord sequence was started. More keys are expected.
    ChordStarted,
    /// A pending chord was cancelled (escape, timeout, or invalid key).
    ChordCancelled,
    /// The key was explicitly unbound (null action).
    Unbound,
    /// No matching binding found.
    None,
}

/// The keybinding resolver. Holds default + custom bindings and the chord
/// state machine.
#[derive(Debug)]
pub struct KeybindingResolver {
    /// All bindings in resolution order. Defaults come first, then custom
    /// bindings. Last match wins (so custom overrides default).
    bindings: Vec<KeyBinding>,
    /// Chord state machine for multi-key sequences.
    chord_matcher: ChordMatcher,
}

impl Default for KeybindingResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeybindingResolver {
    /// Create a new resolver loaded with the default bindings.
    pub fn new() -> Self {
        Self {
            bindings: get_default_bindings(),
            chord_matcher: ChordMatcher::default(),
        }
    }

    /// Create a resolver with specific bindings (useful for testing).
    pub fn with_bindings(bindings: Vec<KeyBinding>) -> Self {
        Self {
            bindings,
            chord_matcher: ChordMatcher::default(),
        }
    }

    /// Load custom bindings from a JSON file, appending them after defaults.
    ///
    /// The file format matches `keybindings.json`:
    /// ```json
    /// {
    ///   "bindings": [
    ///     { "context": "Chat", "bindings": { "ctrl+k": "chat:cancel" } }
    ///   ]
    /// }
    /// ```
    ///
    /// Returns a list of warning strings for invalid entries.
    pub fn load_custom(&mut self, path: &Path) -> Result<Vec<String>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

        let parsed: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("invalid JSON in {}: {}", path.display(), e))?;

        let blocks_value = parsed
            .get("bindings")
            .ok_or_else(|| {
                format!(
                    "keybindings file {} must have a \"bindings\" array",
                    path.display()
                )
            })?
            .clone();

        let blocks: Vec<KeybindingBlock> = serde_json::from_value(blocks_value)
            .map_err(|e| format!("invalid bindings structure in {}: {}", path.display(), e))?;

        let mut warnings = Vec::new();

        // Validate contexts
        for block in &blocks {
            if KeybindingContext::from_str_exact(&block.context).is_none() {
                warnings.push(format!("unknown context: {}", block.context));
            }
        }

        let custom = parse_blocks(&blocks);
        self.bindings.extend(custom);
        Ok(warnings)
    }

    /// Load custom bindings from a JSON string (for testing or embedded config).
    pub fn load_custom_json(&mut self, json: &str) -> Result<Vec<String>, String> {
        let parsed: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid JSON: {}", e))?;

        let blocks_value = parsed
            .get("bindings")
            .ok_or_else(|| "JSON must have a \"bindings\" array".to_string())?
            .clone();

        let blocks: Vec<KeybindingBlock> =
            serde_json::from_value(blocks_value).map_err(|e| format!("invalid blocks: {}", e))?;

        let mut warnings = Vec::new();
        for block in &blocks {
            if KeybindingContext::from_str_exact(&block.context).is_none() {
                warnings.push(format!("unknown context: {}", block.context));
            }
        }

        let custom = parse_blocks(&blocks);
        self.bindings.extend(custom);
        Ok(warnings)
    }

    /// Resolve a key event in the given contexts.
    ///
    /// `active_contexts` should be ordered from most specific to least specific,
    /// e.g., `[Autocomplete, Chat, Global]`. The resolver checks all of them.
    ///
    /// This method handles both single-key bindings and chord sequences. The
    /// chord matcher is stateful -- it remembers partial chord sequences between
    /// calls.
    pub fn resolve(
        &mut self,
        key: &KeyCombo,
        active_contexts: &[KeybindingContext],
    ) -> ResolveResult {
        // First, check chord state
        let chord_result =
            self.chord_matcher
                .process_key(key, &self.bindings, active_contexts);

        match chord_result {
            ChordMatchResult::Match(action) => return ResolveResult::Match(action),
            ChordMatchResult::Pending => return ResolveResult::ChordStarted,
            ChordMatchResult::Cancelled => return ResolveResult::ChordCancelled,
            ChordMatchResult::None => {
                // No chord involvement -- fall through to single-key resolution
            }
        }

        // Single-key resolution (last match wins)
        let ctx_set: HashSet<KeybindingContext> = active_contexts.iter().copied().collect();
        let mut matched: Option<&KeyBinding> = Option::None;

        for binding in &self.bindings {
            if binding.chord.len() != 1 {
                continue;
            }
            if !ctx_set.contains(&binding.context) {
                continue;
            }
            if key.matches(&binding.chord[0]) {
                matched = Some(binding);
            }
        }

        match matched {
            Some(binding) => match &binding.action {
                Some(action) => ResolveResult::Match(action.clone()),
                Option::None => ResolveResult::Unbound,
            },
            Option::None => ResolveResult::None,
        }
    }

    /// Resolve a single key without modifying chord state.
    /// Only checks single-key bindings (no chord support).
    pub fn resolve_simple(
        &self,
        key: &KeyCombo,
        active_contexts: &[KeybindingContext],
    ) -> Option<KeybindingAction> {
        let ctx_set: HashSet<KeybindingContext> = active_contexts.iter().copied().collect();
        let mut result: Option<&KeybindingAction> = None;

        for binding in &self.bindings {
            if binding.chord.len() != 1 {
                continue;
            }
            if !ctx_set.contains(&binding.context) {
                continue;
            }
            if key.matches(&binding.chord[0]) {
                if let Some(action) = &binding.action {
                    result = Some(action);
                } else {
                    result = None; // Null unbind clears previous match
                }
            }
        }

        result.cloned()
    }

    /// Get the display text for an action's shortcut in a given context.
    ///
    /// Searches bindings in reverse order so user overrides take precedence.
    /// Returns `None` if the action is not bound.
    pub fn get_shortcut_display(
        &self,
        action: &KeybindingAction,
        context: KeybindingContext,
    ) -> Option<String> {
        self.bindings
            .iter()
            .rev()
            .find(|b| b.context == context && b.action.as_ref() == Some(action))
            .map(|b| crate::keybindings::bindings::chord_to_string(&b.chord))
    }

    /// Return a reference to all bindings.
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Clear chord state.
    pub fn clear_chord(&mut self) {
        self.chord_matcher.clear();
    }

    /// Whether a chord is currently pending.
    pub fn is_chord_pending(&self) -> bool {
        self.chord_matcher.is_pending()
    }
}

/// Convert a crossterm `KeyEvent` to a `KeyCombo`.
///
/// This is the bridge between crossterm's event model and our keybinding
/// system.
pub fn crossterm_key_to_combo(event: &crossterm::event::KeyEvent) -> KeyCombo {
    use crossterm::event::{KeyCode, KeyModifiers};

    let modifiers = event.modifiers;
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let meta = modifiers.contains(KeyModifiers::META);
    let super_key = modifiers.contains(KeyModifiers::SUPER);

    let key = match event.code {
        KeyCode::Esc => "escape".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => {
            // BackTab is shift+tab; crossterm represents it as a dedicated code
            return KeyCombo {
                key: "tab".to_string(),
                ctrl,
                shift: true, // Always set shift for BackTab
                alt,
                meta,
                super_key,
            };
        }
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::F(n) => format!("f{}", n),
        KeyCode::Char(' ') => " ".to_string(),
        KeyCode::Char(c) => c.to_lowercase().to_string(),
        KeyCode::Null => "null".to_string(),
        _ => return KeyCombo::default(),
    };

    KeyCombo {
        key,
        ctrl,
        shift,
        alt,
        meta,
        super_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::actions::KeybindingAction;
    use crate::keybindings::bindings::KeyCombo;

    #[test]
    fn test_resolve_simple_global() {
        let resolver = KeybindingResolver::new();
        let result =
            resolver.resolve_simple(&KeyCombo::ctrl("c"), &[KeybindingContext::Global]);
        assert_eq!(result, Some(KeybindingAction::AppInterrupt));
    }

    #[test]
    fn test_resolve_simple_chat() {
        let resolver = KeybindingResolver::new();
        let result = resolver.resolve_simple(
            &KeyCombo::key("enter"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, Some(KeybindingAction::ChatSubmit));
    }

    #[test]
    fn test_resolve_context_priority() {
        let resolver = KeybindingResolver::new();

        // escape in Autocomplete -> AutocompleteDismiss
        let result = resolver.resolve_simple(
            &KeyCombo::key("escape"),
            &[KeybindingContext::Autocomplete, KeybindingContext::Global],
        );
        assert_eq!(result, Some(KeybindingAction::AutocompleteDismiss));

        // escape in Chat -> ChatCancel
        let result = resolver.resolve_simple(
            &KeyCombo::key("escape"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, Some(KeybindingAction::ChatCancel));
    }

    #[test]
    fn test_resolve_no_match() {
        let resolver = KeybindingResolver::new();
        let result = resolver.resolve_simple(
            &KeyCombo::key("z"),
            &[KeybindingContext::Global],
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_with_chord() {
        let mut resolver = KeybindingResolver::new();

        // First key of chord: ctrl+x
        let result = resolver.resolve(
            &KeyCombo::ctrl("x"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, ResolveResult::ChordStarted);

        // Second key: ctrl+k -> ChatKillAgents
        let result = resolver.resolve(
            &KeyCombo::ctrl("k"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(
            result,
            ResolveResult::Match(KeybindingAction::ChatKillAgents)
        );
    }

    #[test]
    fn test_custom_override() {
        let mut resolver = KeybindingResolver::new();

        // Override ctrl+l in Global to ChatCancel
        let warnings = resolver
            .load_custom_json(
                r#"{
                    "bindings": [{
                        "context": "Global",
                        "bindings": { "ctrl+l": "chat:cancel" }
                    }]
                }"#,
            )
            .unwrap();
        assert!(warnings.is_empty());

        let result =
            resolver.resolve_simple(&KeyCombo::ctrl("l"), &[KeybindingContext::Global]);
        assert_eq!(result, Some(KeybindingAction::ChatCancel));
    }

    #[test]
    fn test_custom_unbind() {
        let mut resolver = KeybindingResolver::new();

        // Unbind ctrl+l in Global
        let warnings = resolver
            .load_custom_json(
                r#"{
                    "bindings": [{
                        "context": "Global",
                        "bindings": { "ctrl+l": null }
                    }]
                }"#,
            )
            .unwrap();
        assert!(warnings.is_empty());

        let result =
            resolver.resolve_simple(&KeyCombo::ctrl("l"), &[KeybindingContext::Global]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_shortcut_display() {
        let resolver = KeybindingResolver::new();

        let display = resolver.get_shortcut_display(
            &KeybindingAction::AppInterrupt,
            KeybindingContext::Global,
        );
        assert_eq!(display, Some("ctrl+c".to_string()));

        let display = resolver.get_shortcut_display(
            &KeybindingAction::ChatSubmit,
            KeybindingContext::Chat,
        );
        assert_eq!(display, Some("Enter".to_string()));
    }

    #[test]
    fn test_crossterm_key_conversion() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let combo = crossterm_key_to_combo(&event);
        assert_eq!(combo.key, "c");
        assert!(combo.ctrl);
        assert!(!combo.shift);

        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let combo = crossterm_key_to_combo(&event);
        assert_eq!(combo.key, "escape");
        assert!(!combo.ctrl);

        let event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        let combo = crossterm_key_to_combo(&event);
        assert_eq!(combo.key, "tab");
        assert!(combo.shift);
    }

    #[test]
    fn test_resolve_confirmation_keys() {
        let resolver = KeybindingResolver::new();

        let result = resolver.resolve_simple(
            &KeyCombo::key("y"),
            &[KeybindingContext::Confirmation],
        );
        assert_eq!(result, Some(KeybindingAction::ConfirmYes));

        let result = resolver.resolve_simple(
            &KeyCombo::key("n"),
            &[KeybindingContext::Confirmation],
        );
        assert_eq!(result, Some(KeybindingAction::ConfirmNo));
    }

    #[test]
    fn test_resolve_history_search() {
        let resolver = KeybindingResolver::new();

        // ctrl+r in HistorySearch -> HistorySearchNext
        let result = resolver.resolve_simple(
            &KeyCombo::ctrl("r"),
            &[KeybindingContext::HistorySearch, KeybindingContext::Global],
        );
        // Last match wins: HistorySearch context's ctrl+r (HistorySearchNext)
        // comes after Global's ctrl+r (HistorySearch)
        assert_eq!(result, Some(KeybindingAction::HistorySearchNext));
    }

    #[test]
    fn test_resolve_tab_in_autocomplete_vs_confirmation() {
        let resolver = KeybindingResolver::new();

        let result = resolver.resolve_simple(
            &KeyCombo::key("tab"),
            &[KeybindingContext::Autocomplete],
        );
        assert_eq!(result, Some(KeybindingAction::AutocompleteAccept));

        let result = resolver.resolve_simple(
            &KeyCombo::key("tab"),
            &[KeybindingContext::Confirmation],
        );
        assert_eq!(result, Some(KeybindingAction::ConfirmNextField));
    }

    #[test]
    fn test_resolve_multiple_contexts() {
        let resolver = KeybindingResolver::new();

        // When both Autocomplete and Chat are active, escape should resolve
        // to the last match (which context's binding appears later in the list)
        let result = resolver.resolve_simple(
            &KeyCombo::key("escape"),
            &[
                KeybindingContext::Autocomplete,
                KeybindingContext::Chat,
                KeybindingContext::Global,
            ],
        );
        // Autocomplete's escape comes after Chat's escape in default bindings
        assert_eq!(result, Some(KeybindingAction::AutocompleteDismiss));
    }

    #[test]
    fn test_chord_then_single_key() {
        let mut resolver = KeybindingResolver::new();

        // Start a chord
        let result = resolver.resolve(
            &KeyCombo::ctrl("x"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, ResolveResult::ChordStarted);

        // Cancel with escape
        let result = resolver.resolve(
            &KeyCombo::key("escape"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, ResolveResult::ChordCancelled);

        // Now a single key should work normally
        let result = resolver.resolve(
            &KeyCombo::key("enter"),
            &[KeybindingContext::Chat, KeybindingContext::Global],
        );
        assert_eq!(result, ResolveResult::Match(KeybindingAction::ChatSubmit));
    }

    #[test]
    fn test_select_context_bindings() {
        let resolver = KeybindingResolver::new();

        let result = resolver
            .resolve_simple(&KeyCombo::key("j"), &[KeybindingContext::Select]);
        assert_eq!(result, Some(KeybindingAction::SelectNext));

        let result = resolver
            .resolve_simple(&KeyCombo::key("k"), &[KeybindingContext::Select]);
        assert_eq!(result, Some(KeybindingAction::SelectPrevious));
    }

    #[test]
    fn test_diff_dialog_bindings() {
        let resolver = KeybindingResolver::new();

        let result = resolver.resolve_simple(
            &KeyCombo::key("escape"),
            &[KeybindingContext::DiffDialog],
        );
        assert_eq!(result, Some(KeybindingAction::DiffDismiss));

        let result = resolver.resolve_simple(
            &KeyCombo::key("left"),
            &[KeybindingContext::DiffDialog],
        );
        assert_eq!(result, Some(KeybindingAction::DiffPreviousSource));
    }

    #[test]
    fn test_message_selector_bindings() {
        let resolver = KeybindingResolver::new();

        let result = resolver.resolve_simple(
            &KeyCombo::ctrl("up"),
            &[KeybindingContext::MessageSelector],
        );
        assert_eq!(result, Some(KeybindingAction::MessageSelectorTop));

        let result = resolver.resolve_simple(
            &KeyCombo::shift("j"),
            &[KeybindingContext::MessageSelector],
        );
        assert_eq!(result, Some(KeybindingAction::MessageSelectorBottom));
    }
}
