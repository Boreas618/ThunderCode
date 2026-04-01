//! Multi-key chord binding support.
//!
//! Chord sequences (e.g., `ctrl+x ctrl+e`) require tracking intermediate
//! state between keystrokes. This module provides [`ChordMatcher`] which
//! manages that state and a timeout so stale prefixes expire.
//!
//! Ported from the chord logic in `resolver.ts`'s `resolveKeyWithChordState`.

use std::time::{Duration, Instant};

use crate::keybindings::actions::KeybindingAction;
use crate::keybindings::bindings::{KeyBinding, KeyCombo};
use crate::keybindings::context::KeybindingContext;

/// A chord binding is a multi-key sequence mapped to an action.
#[derive(Debug, Clone, PartialEq)]
pub struct ChordBinding {
    /// The full key sequence (2+ elements).
    pub sequence: Vec<KeyCombo>,
    /// The action to trigger when the full sequence is matched.
    pub action: KeybindingAction,
    /// The context in which this chord is active.
    pub context: KeybindingContext,
}

impl ChordBinding {
    /// Create a `ChordBinding` from a `KeyBinding`, returning `None` if the
    /// binding is a single-key binding or has no action.
    pub fn from_key_binding(binding: &KeyBinding) -> Option<Self> {
        if binding.chord.len() < 2 {
            return None;
        }
        Some(Self {
            sequence: binding.chord.clone(),
            action: binding.action.clone()?,
            context: binding.context,
        })
    }
}

/// Result of chord matching.
#[derive(Debug, Clone, PartialEq)]
pub enum ChordMatchResult {
    /// A complete chord matched. Contains the resolved action.
    Match(KeybindingAction),
    /// The key started (or continued) a chord. More keys expected.
    Pending,
    /// A pending chord was cancelled (escape, timeout, or invalid key).
    Cancelled,
    /// No chord involvement -- the key should be resolved as a single-key binding.
    None,
}

/// Manages chord state across keystrokes.
///
/// Keeps track of the keys pressed so far and a timeout. If the timeout
/// elapses between keystrokes, the pending chord is automatically cancelled.
#[derive(Debug)]
pub struct ChordMatcher {
    /// Keys pressed so far in the current chord attempt.
    pending: Vec<KeyCombo>,
    /// When the last key of the pending sequence was pressed.
    last_key_time: Option<Instant>,
    /// Maximum time between chord keystrokes before the chord is cancelled.
    timeout: Duration,
}

impl Default for ChordMatcher {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl ChordMatcher {
    /// Create a new `ChordMatcher` with the given timeout in milliseconds.
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            pending: Vec::new(),
            last_key_time: None,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Whether a chord is currently in progress.
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The pending key sequence (empty if no chord in progress).
    pub fn pending_keys(&self) -> &[KeyCombo] {
        &self.pending
    }

    /// Clear any pending chord state.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.last_key_time = None;
    }

    /// Process a key event and determine the chord match result.
    ///
    /// `bindings` is the full binding set (including single-key bindings);
    /// only multi-key (chord) bindings are checked for prefix / exact matches.
    /// `active_contexts` controls which contexts are considered.
    ///
    /// The logic matches `resolveKeyWithChordState` from `resolver.ts`:
    /// 1. Escape while pending cancels the chord.
    /// 2. Check for timeout -- cancel if expired.
    /// 3. Build the test chord (pending + current key).
    /// 4. If any longer chord starts with this prefix, return Pending.
    /// 5. If an exact chord match is found, return Match.
    /// 6. Otherwise cancel if we were pending, or return None.
    pub fn process_key(
        &mut self,
        key: &KeyCombo,
        bindings: &[KeyBinding],
        active_contexts: &[KeybindingContext],
    ) -> ChordMatchResult {
        // Escape while pending cancels
        if key.key == "escape" && self.is_pending() {
            self.clear();
            return ChordMatchResult::Cancelled;
        }

        // Check timeout
        if let Some(last_time) = self.last_key_time {
            if last_time.elapsed() > self.timeout {
                let was_pending = self.is_pending();
                self.clear();
                if was_pending {
                    // Timed out -- process this key fresh (fall through below)
                }
            }
        }

        // Build the test chord
        let mut test_chord: Vec<KeyCombo> = self.pending.clone();
        test_chord.push(key.clone());

        // Filter bindings by active contexts (only chord bindings, length > 1)
        let ctx_set: std::collections::HashSet<KeybindingContext> =
            active_contexts.iter().copied().collect();

        // Check for prefix matches that would extend the chord.
        // Null-overrides shadow the binding they unbind (same as TS).
        let mut chord_winners: std::collections::HashMap<String, Option<&KeybindingAction>> =
            std::collections::HashMap::new();

        for binding in bindings {
            if binding.chord.len() <= test_chord.len() {
                continue;
            }
            if !ctx_set.contains(&binding.context) {
                continue;
            }
            if chord_prefix_matches(&test_chord, &binding.chord) {
                let chord_key = chord_to_key(&binding.chord);
                chord_winners.insert(chord_key, binding.action.as_ref());
            }
        }

        let has_longer = chord_winners.values().any(|a| a.is_some());

        if has_longer {
            self.pending = test_chord;
            self.last_key_time = Some(Instant::now());
            return ChordMatchResult::Pending;
        }

        // Check for exact matches (last one wins).
        // Only consider multi-key bindings (length >= 2). Single-key bindings
        // are handled by the resolver's single-key path, not the chord matcher.
        let mut exact_match: Option<&KeyBinding> = None;
        for binding in bindings {
            if binding.chord.len() < 2 {
                continue;
            }
            if !ctx_set.contains(&binding.context) {
                continue;
            }
            if chord_exactly_matches(&test_chord, &binding.chord) {
                exact_match = Some(binding);
            }
        }

        if let Some(matched) = exact_match {
            self.clear();
            if let Some(action) = &matched.action {
                return ChordMatchResult::Match(action.clone());
            }
            // Null action = unbound
            return ChordMatchResult::Cancelled;
        }

        // No match found
        if self.is_pending() {
            self.clear();
            return ChordMatchResult::Cancelled;
        }

        ChordMatchResult::None
    }
}

/// Check if `prefix` is a prefix of `full_chord`.
fn chord_prefix_matches(prefix: &[KeyCombo], full_chord: &[KeyCombo]) -> bool {
    if prefix.len() >= full_chord.len() {
        return false;
    }
    for (p, f) in prefix.iter().zip(full_chord.iter()) {
        if !p.matches(f) {
            return false;
        }
    }
    true
}

/// Check if two chord sequences match exactly.
fn chord_exactly_matches(a: &[KeyCombo], b: &[KeyCombo]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (ak, bk) in a.iter().zip(b.iter()) {
        if !ak.matches(bk) {
            return false;
        }
    }
    true
}

/// Build a string key for deduplication (used in chord_winners map).
fn chord_to_key(chord: &[KeyCombo]) -> String {
    crate::keybindings::bindings::chord_to_string(chord)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::actions::KeybindingAction;
    use crate::keybindings::bindings::KeyBinding;

    fn make_chord_binding(
        keys: Vec<KeyCombo>,
        action: KeybindingAction,
        ctx: KeybindingContext,
    ) -> KeyBinding {
        KeyBinding {
            chord: keys,
            action: Some(action),
            context: ctx,
        }
    }

    fn make_single_binding(
        key: KeyCombo,
        action: KeybindingAction,
        ctx: KeybindingContext,
    ) -> KeyBinding {
        KeyBinding {
            chord: vec![key],
            action: Some(action),
            context: ctx,
        }
    }

    #[test]
    fn test_single_key_no_chord() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![make_single_binding(
            KeyCombo::ctrl("c"),
            KeybindingAction::AppInterrupt,
            KeybindingContext::Global,
        )];

        let result = matcher.process_key(
            &KeyCombo::ctrl("c"),
            &bindings,
            &[KeybindingContext::Global],
        );
        assert_eq!(result, ChordMatchResult::None);
    }

    #[test]
    fn test_chord_two_keys() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![make_chord_binding(
            vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
            KeybindingAction::ChatKillAgents,
            KeybindingContext::Chat,
        )];

        // First key: prefix match -> Pending
        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Pending);
        assert!(matcher.is_pending());

        // Second key: exact match -> Match
        let result =
            matcher.process_key(&KeyCombo::ctrl("k"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(
            result,
            ChordMatchResult::Match(KeybindingAction::ChatKillAgents)
        );
        assert!(!matcher.is_pending());
    }

    #[test]
    fn test_chord_escape_cancels() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![make_chord_binding(
            vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
            KeybindingAction::ChatKillAgents,
            KeybindingContext::Chat,
        )];

        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Pending);

        let result = matcher.process_key(
            &KeyCombo::key("escape"),
            &bindings,
            &[KeybindingContext::Chat],
        );
        assert_eq!(result, ChordMatchResult::Cancelled);
        assert!(!matcher.is_pending());
    }

    #[test]
    fn test_chord_wrong_second_key_cancels() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![make_chord_binding(
            vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
            KeybindingAction::ChatKillAgents,
            KeybindingContext::Chat,
        )];

        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Pending);

        // Wrong second key
        let result =
            matcher.process_key(&KeyCombo::ctrl("z"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Cancelled);
    }

    #[test]
    fn test_chord_context_filtering() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![make_chord_binding(
            vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
            KeybindingAction::ChatKillAgents,
            KeybindingContext::Chat,
        )];

        // Wrong context: Global instead of Chat -- chord not found
        let result = matcher.process_key(
            &KeyCombo::ctrl("x"),
            &bindings,
            &[KeybindingContext::Global],
        );
        assert_eq!(result, ChordMatchResult::None);
    }

    #[test]
    fn test_chord_timeout() {
        let mut matcher = ChordMatcher::new(0); // 0ms timeout = instant expiry
        let bindings = vec![make_chord_binding(
            vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
            KeybindingAction::ChatKillAgents,
            KeybindingContext::Chat,
        )];

        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Pending);

        // Sleep a tiny bit to ensure timeout
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Timeout expired -- chord cancelled, key processed fresh
        let result =
            matcher.process_key(&KeyCombo::ctrl("k"), &bindings, &[KeybindingContext::Chat]);
        // ctrl+k alone doesn't match any chord, so None
        assert_eq!(result, ChordMatchResult::None);
    }

    #[test]
    fn test_chord_binding_from_key_binding() {
        let kb = KeyBinding {
            chord: vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("e")],
            action: Some(KeybindingAction::ChatExternalEditor),
            context: KeybindingContext::Chat,
        };
        let cb = ChordBinding::from_key_binding(&kb).unwrap();
        assert_eq!(cb.sequence.len(), 2);
        assert_eq!(cb.action, KeybindingAction::ChatExternalEditor);

        // Single-key binding returns None
        let single = KeyBinding {
            chord: vec![KeyCombo::key("enter")],
            action: Some(KeybindingAction::ChatSubmit),
            context: KeybindingContext::Chat,
        };
        assert!(ChordBinding::from_key_binding(&single).is_none());
    }

    #[test]
    fn test_two_chords_same_prefix() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![
            make_chord_binding(
                vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
                KeybindingAction::ChatKillAgents,
                KeybindingContext::Chat,
            ),
            make_chord_binding(
                vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("e")],
                KeybindingAction::ChatExternalEditor,
                KeybindingContext::Chat,
            ),
        ];

        // First key is prefix for both
        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::Pending);

        // Completing the first chord
        let result =
            matcher.process_key(&KeyCombo::ctrl("k"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(
            result,
            ChordMatchResult::Match(KeybindingAction::ChatKillAgents)
        );
    }

    #[test]
    fn test_null_override_shadows_chord() {
        let mut matcher = ChordMatcher::default();
        let bindings = vec![
            // Default chord
            make_chord_binding(
                vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
                KeybindingAction::ChatKillAgents,
                KeybindingContext::Chat,
            ),
            // User null-unbinds it (action = None)
            KeyBinding {
                chord: vec![KeyCombo::ctrl("x"), KeyCombo::ctrl("k")],
                action: None,
                context: KeybindingContext::Chat,
            },
        ];

        // ctrl+x should NOT enter pending because the only longer chord is unbound
        let result =
            matcher.process_key(&KeyCombo::ctrl("x"), &bindings, &[KeybindingContext::Chat]);
        assert_eq!(result, ChordMatchResult::None);
    }
}
