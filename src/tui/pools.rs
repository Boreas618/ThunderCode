//! String and style interning pools for memory-efficient screen buffers.
//!
//! Pools are shared across screens so interned IDs are valid across front/back
//! buffers. This enables zero-copy blit and integer-only diff comparisons.

use std::collections::HashMap;

use crate::tui::style::AnsiCode;

// ---------------------------------------------------------------------------
// CharPool
// ---------------------------------------------------------------------------

/// Character string pool shared across all screens.
///
/// Index 0 = `' '` (space), Index 1 = `""` (empty, used for spacer cells).
/// ASCII characters get a fast-path via a lookup table.
pub struct CharPool {
    strings: Vec<String>,
    string_map: HashMap<String, u32>,
    /// ASCII fast-path: `char_code -> index`, `u32::MAX` = not interned.
    ascii: [u32; 128],
}

impl CharPool {
    pub fn new() -> Self {
        let mut ascii = [u32::MAX; 128];
        ascii[b' ' as usize] = 0; // space is index 0
        Self {
            strings: vec![" ".into(), String::new()], // 0 = space, 1 = empty
            string_map: HashMap::from([(" ".into(), 0), (String::new(), 1)]),
            ascii,
        }
    }

    /// Intern a character string. Returns an integer ID valid for the pool's lifetime.
    pub fn intern(&mut self, ch: &str) -> u32 {
        // ASCII fast-path
        if ch.len() == 1 {
            let code = ch.as_bytes()[0];
            if code < 128 {
                let cached = self.ascii[code as usize];
                if cached != u32::MAX {
                    return cached;
                }
                let index = self.strings.len() as u32;
                self.strings.push(ch.into());
                self.ascii[code as usize] = index;
                return index;
            }
        }
        if let Some(&existing) = self.string_map.get(ch) {
            return existing;
        }
        let index = self.strings.len() as u32;
        self.strings.push(ch.into());
        self.string_map.insert(ch.into(), index);
        index
    }

    /// Retrieve the string for a given ID. Returns `" "` for unknown IDs.
    #[inline]
    pub fn get(&self, index: u32) -> &str {
        self.strings
            .get(index as usize)
            .map(|s| s.as_str())
            .unwrap_or(" ")
    }
}

impl Default for CharPool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HyperlinkPool
// ---------------------------------------------------------------------------

/// Hyperlink string pool. Index 0 = no hyperlink.
pub struct HyperlinkPool {
    strings: Vec<String>,
    string_map: HashMap<String, u32>,
}

impl HyperlinkPool {
    pub fn new() -> Self {
        Self {
            strings: vec![String::new()], // 0 = no hyperlink
            string_map: HashMap::new(),
        }
    }

    /// Intern a hyperlink URL. `None` or empty returns 0.
    pub fn intern(&mut self, hyperlink: Option<&str>) -> u32 {
        match hyperlink {
            None | Some("") => 0,
            Some(url) => {
                if let Some(&id) = self.string_map.get(url) {
                    return id;
                }
                let id = self.strings.len() as u32;
                self.strings.push(url.into());
                self.string_map.insert(url.into(), id);
                id
            }
        }
    }

    /// Retrieve the URL for a given ID. Returns `None` for 0 or unknown IDs.
    #[inline]
    pub fn get(&self, id: u32) -> Option<&str> {
        if id == 0 {
            None
        } else {
            self.strings.get(id as usize).map(|s| s.as_str())
        }
    }
}

impl Default for HyperlinkPool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StylePool
// ---------------------------------------------------------------------------

/// SGR style interning pool with transition caching.
///
/// **Bit 0 of the returned ID encodes whether the style has a visible effect
/// on space characters** (background, inverse, underline, strikethrough,
/// overline). Foreground-only styles get even IDs; styles visible on spaces
/// get odd IDs. This lets the renderer skip invisible spaces with a single
/// bitmask check.
pub struct StylePool {
    styles: Vec<Vec<AnsiCode>>,
    ids: HashMap<String, u32>,
    /// Cache of `(from_id, to_id) -> ANSI transition string`.
    transition_cache: HashMap<u64, String>,
    /// The ID returned by `intern(&[])` -- always the first entry.
    pub none: u32,
}

impl StylePool {
    pub fn new() -> Self {
        let mut pool = Self {
            styles: Vec::new(),
            ids: HashMap::new(),
            transition_cache: HashMap::new(),
            none: 0,
        };
        pool.none = pool.intern(&[]);
        pool
    }

    /// Intern a style (sequence of ANSI codes) and return its ID.
    pub fn intern(&mut self, codes: &[AnsiCode]) -> u32 {
        let key = if codes.is_empty() {
            String::new()
        } else {
            codes
                .iter()
                .map(|c| c.code.as_str())
                .collect::<Vec<_>>()
                .join("\0")
        };

        if let Some(&id) = self.ids.get(&key) {
            return id;
        }

        let raw_id = self.styles.len() as u32;
        self.styles.push(codes.to_vec());

        let visible = !codes.is_empty() && has_visible_space_effect(codes);
        let id = (raw_id << 1) | (if visible { 1 } else { 0 });
        self.ids.insert(key, id);
        id
    }

    /// Recover styles from an encoded ID. Strips the bit-0 flag via `>> 1`.
    #[inline]
    pub fn get(&self, id: u32) -> &[AnsiCode] {
        self.styles
            .get((id >> 1) as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Pre-serialized ANSI string to transition from one style to another.
    /// Cached by `(from_id, to_id)` -- zero allocations after first call.
    pub fn transition(&mut self, from_id: u32, to_id: u32) -> &str {
        if from_id == to_id {
            return "";
        }
        let key = (from_id as u64) * 0x100000 + (to_id as u64);
        // We need to compute the transition if not cached. Use entry API.
        if !self.transition_cache.contains_key(&key) {
            let from_codes = self.get(from_id).to_vec();
            let to_codes = self.get(to_id).to_vec();
            let diff = diff_ansi_codes(&from_codes, &to_codes);
            self.transition_cache.insert(key, diff);
        }
        self.transition_cache.get(&key).unwrap()
    }

    /// Intern a style that is `base + SGR 7 (inverse)`.
    pub fn with_inverse(&mut self, base_id: u32) -> u32 {
        let base_codes = self.get(base_id).to_vec();
        let has_inverse = base_codes.iter().any(|c| c.end_code == "\x1b[27m");
        if has_inverse {
            return base_id;
        }
        let mut codes = base_codes;
        codes.push(AnsiCode::new("\x1b[7m", "\x1b[27m"));
        self.intern(&codes)
    }
}

impl Default for StylePool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// End codes that produce visible effects on space characters.
const VISIBLE_ON_SPACE: &[&str] = &[
    "\x1b[49m", // background color
    "\x1b[27m", // inverse
    "\x1b[24m", // underline
    "\x1b[29m", // strikethrough
    "\x1b[55m", // overline
];

fn has_visible_space_effect(codes: &[AnsiCode]) -> bool {
    codes
        .iter()
        .any(|c| VISIBLE_ON_SPACE.contains(&c.end_code.as_str()))
}

/// Compute the ANSI string to transition from one style to another.
/// If the target is empty, emit a reset. Otherwise emit the diff.
fn diff_ansi_codes(from: &[AnsiCode], to: &[AnsiCode]) -> String {
    if to.is_empty() {
        if from.is_empty() {
            return String::new();
        }
        return "\x1b[0m".into();
    }

    // Simple approach: check what needs to be turned off and on.
    // Codes present in `from` but not in `to` need their end_code.
    // Codes present in `to` but not in `from` need their code.
    let mut result = String::new();

    // Find codes to remove (in `from` but not in `to`)
    let mut needs_reset = false;
    for fc in from {
        if !to.iter().any(|tc| tc.code == fc.code) {
            // This code is being removed. Some SGR attributes can only be
            // turned off via their specific end code, but if we have many
            // removals it's simpler to reset and re-apply.
            needs_reset = true;
            break;
        }
    }

    if needs_reset && !from.is_empty() {
        result.push_str("\x1b[0m");
        // After reset, apply all target codes
        for tc in to {
            result.push_str(&tc.code);
        }
    } else {
        // Only add new codes not present in from
        for tc in to {
            if !from.iter().any(|fc| fc.code == tc.code) {
                result.push_str(&tc.code);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_pool_ascii_fast_path() {
        let mut pool = CharPool::new();
        let id_a = pool.intern("a");
        let id_a2 = pool.intern("a");
        assert_eq!(id_a, id_a2);
        assert_eq!(pool.get(id_a), "a");
    }

    #[test]
    fn test_char_pool_space_and_empty() {
        let pool = CharPool::new();
        assert_eq!(pool.get(0), " ");
        assert_eq!(pool.get(1), "");
    }

    #[test]
    fn test_char_pool_unicode() {
        let mut pool = CharPool::new();
        let id = pool.intern("\u{1F600}"); // grinning face emoji
        assert_eq!(pool.get(id), "\u{1F600}");
        // Re-interning returns same ID
        assert_eq!(pool.intern("\u{1F600}"), id);
    }

    #[test]
    fn test_hyperlink_pool() {
        let mut pool = HyperlinkPool::new();
        assert_eq!(pool.intern(None), 0);
        assert_eq!(pool.intern(Some("")), 0);
        assert!(pool.get(0).is_none());

        let id = pool.intern(Some("https://example.com"));
        assert!(id > 0);
        assert_eq!(pool.get(id), Some("https://example.com"));
        assert_eq!(pool.intern(Some("https://example.com")), id);
    }

    #[test]
    fn test_style_pool_none() {
        let pool = StylePool::new();
        assert_eq!(pool.none, 0);
        assert!(pool.get(0).is_empty());
    }

    #[test]
    fn test_style_pool_visible_flag() {
        let mut pool = StylePool::new();

        // Foreground-only style -> even ID
        let fg_id = pool.intern(&[AnsiCode::new("\x1b[31m", "\x1b[39m")]);
        assert_eq!(fg_id & 1, 0, "fg-only style should have even ID");

        // Background style -> odd ID (visible on spaces)
        let bg_id = pool.intern(&[AnsiCode::new("\x1b[41m", "\x1b[49m")]);
        assert_eq!(bg_id & 1, 1, "bg style should have odd ID");
    }

    #[test]
    fn test_style_pool_transition() {
        let mut pool = StylePool::new();
        let id_a = pool.intern(&[AnsiCode::new("\x1b[1m", "\x1b[22m")]);
        let id_b = pool.intern(&[AnsiCode::new("\x1b[3m", "\x1b[23m")]);

        // Same style -> empty transition
        assert_eq!(pool.transition(id_a, id_a), "");

        // Different styles -> non-empty transition
        let trans = pool.transition(id_a, id_b);
        assert!(!trans.is_empty());
    }

    #[test]
    fn test_style_pool_with_inverse() {
        let mut pool = StylePool::new();
        let base = pool.intern(&[AnsiCode::new("\x1b[31m", "\x1b[39m")]);
        let inv = pool.with_inverse(base);
        assert_ne!(base, inv);

        // Already-inverted base should return same ID
        let already_inv =
            pool.intern(&[AnsiCode::new("\x1b[7m", "\x1b[27m")]);
        let inv2 = pool.with_inverse(already_inv);
        assert_eq!(already_inv, inv2);
    }
}
