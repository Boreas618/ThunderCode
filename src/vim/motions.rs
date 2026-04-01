//! Vim motion functions.
//!
//! Pure functions for resolving vim motions to cursor positions.
//! All functions operate on `(text, offset)` and return a new offset.

use crate::vim::text_objects::{is_vim_whitespace, is_vim_word_char};
use crate::vim::types::FindType;

/// Resolve a motion key with a repeat count, returning the new cursor offset.
/// Stops early if the cursor doesn't move (boundary reached).
pub fn resolve_motion(text: &str, offset: usize, key: &str, count: usize) -> usize {
    let mut result = offset;
    for _ in 0..count {
        let next = apply_single_motion(text, result, key);
        if next == result {
            break;
        }
        result = next;
    }
    result
}

/// Apply a single motion step, returning the new offset.
fn apply_single_motion(text: &str, offset: usize, key: &str) -> usize {
    match key {
        "h" => move_left(text, offset),
        "l" => move_right(text, offset),
        "j" => move_down_logical(text, offset),
        "k" => move_up_logical(text, offset),
        "gj" => move_down_logical(text, offset), // simplified: same as j for us
        "gk" => move_up_logical(text, offset),   // simplified: same as k for us
        "w" => next_vim_word(text, offset),
        "b" => prev_vim_word(text, offset),
        "e" => end_of_vim_word(text, offset),
        "W" => next_word_big(text, offset),
        "B" => prev_word_big(text, offset),
        "E" => end_of_word_big(text, offset),
        "0" => start_of_logical_line(text, offset),
        "^" => first_non_blank_in_line(text, offset),
        "$" => end_of_logical_line(text, offset),
        "G" => start_of_last_line(text),
        _ => offset,
    }
}

/// Check if a motion is inclusive (includes character at destination).
pub fn is_inclusive_motion(key: &str) -> bool {
    matches!(key, "e" | "E" | "$")
}

/// Check if a motion is linewise (operates on full lines when used with operators).
pub fn is_linewise_motion(key: &str) -> bool {
    matches!(key, "j" | "k" | "G" | "gg")
}

// ============================================================================
// Basic movement
// ============================================================================

/// Move cursor left one character, clamped to current line start.
pub fn move_left(text: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    // Find the char index that contains offset
    for (i, &(byte_pos, _)) in chars.iter().enumerate() {
        if byte_pos >= offset {
            // Previous char
            let prev = if i > 0 { chars[i - 1].0 } else { 0 };
            // Don't cross newline boundary backward
            let line_start = start_of_logical_line(text, offset);
            return prev.max(line_start);
        }
    }
    // offset is at/past end
    if let Some(&(byte_pos, _)) = chars.last() {
        let line_start = start_of_logical_line(text, offset);
        return byte_pos.max(line_start);
    }
    0
}

/// Move cursor right one character, clamped to current line end.
pub fn move_right(text: &str, offset: usize) -> usize {
    let line_end = end_of_logical_line(text, offset);
    if offset >= line_end {
        return offset;
    }
    // Advance by one char
    next_char_offset(text, offset).unwrap_or(offset)
}

/// Get the byte offset of the next character after `offset`.
fn next_char_offset(text: &str, offset: usize) -> Option<usize> {
    let rest = text.get(offset..)?;
    let mut chars = rest.chars();
    let ch = chars.next()?;
    Some(offset + ch.len_utf8())
}

/// Get the byte offset of the character before `offset`.
fn prev_char_boundary(text: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut pos = offset - 1;
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

// ============================================================================
// Line movement
// ============================================================================

/// Move down one logical line, trying to preserve column.
pub fn move_down_logical(text: &str, offset: usize) -> usize {
    let line_start = start_of_logical_line(text, offset);
    let col = offset - line_start;

    // Find start of next line
    let rest = &text[offset..];
    if let Some(nl_pos) = rest.find('\n') {
        let next_line_start = offset + nl_pos + 1;
        let next_line_end = end_of_logical_line(text, next_line_start);
        let next_line_len = next_line_end - next_line_start;
        return next_line_start + col.min(next_line_len);
    }
    // Already on last line
    offset
}

/// Move up one logical line, trying to preserve column.
pub fn move_up_logical(text: &str, offset: usize) -> usize {
    let line_start = start_of_logical_line(text, offset);
    if line_start == 0 {
        return offset; // Already on first line
    }
    let col = offset - line_start;

    // Previous line ends at line_start - 1 (the '\n')
    let prev_line_end_offset = line_start - 1;
    let prev_line_start = start_of_logical_line(text, prev_line_end_offset);
    let prev_line_end = end_of_logical_line(text, prev_line_start);
    let prev_line_len = prev_line_end - prev_line_start;
    prev_line_start + col.min(prev_line_len)
}

/// Start of the logical line containing offset.
pub fn start_of_logical_line(text: &str, offset: usize) -> usize {
    let before = &text[..offset];
    match before.rfind('\n') {
        Some(pos) => pos + 1,
        None => 0,
    }
}

/// End of the logical line containing offset (offset of last char, not the newline).
/// For an empty line, returns line_start.
pub fn end_of_logical_line(text: &str, offset: usize) -> usize {
    let rest = &text[offset..];
    let line_end_byte = match rest.find('\n') {
        Some(pos) => offset + pos,
        None => text.len(),
    };
    // end_of_line is the offset of the last char on the line
    let line_start = start_of_logical_line(text, offset);
    if line_end_byte <= line_start {
        return line_start;
    }
    // Go back one char from the newline/end
    prev_char_boundary(text, line_end_byte)
}

/// First non-blank character in the logical line.
pub fn first_non_blank_in_line(text: &str, offset: usize) -> usize {
    let line_start = start_of_logical_line(text, offset);
    let rest = &text[line_start..];
    for (i, ch) in rest.char_indices() {
        if ch == '\n' {
            return line_start;
        }
        if !ch.is_whitespace() {
            return line_start + i;
        }
    }
    line_start
}

/// Start of the first line in the text.
pub fn start_of_first_line(_text: &str) -> usize {
    0
}

/// Start of the last line in the text.
pub fn start_of_last_line(text: &str) -> usize {
    match text.rfind('\n') {
        Some(pos) => pos + 1,
        None => 0,
    }
}

/// Go to line N (1-based), returning the offset of the start of that line.
pub fn go_to_line(text: &str, line_num: usize) -> usize {
    if line_num <= 1 {
        return 0;
    }
    let target = line_num - 1; // zero-based
    let mut current_line = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == target {
                return i + 1;
            }
        }
    }
    // If line_num exceeds total lines, go to last line
    start_of_last_line(text)
}

// ============================================================================
// Word motions
// ============================================================================

/// Move to the start of the next vim word.
pub fn next_vim_word(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();

    // Find char index for our byte offset
    let mut idx = match chars.iter().position(|&(b, _)| b >= offset) {
        Some(i) => i,
        None => return offset,
    };

    if idx >= len {
        return offset;
    }

    let cur_ch = chars[idx].1;

    // Skip current word/punct/whitespace block
    if is_vim_word_char(cur_ch) {
        while idx < len && is_vim_word_char(chars[idx].1) {
            idx += 1;
        }
    } else if !is_vim_whitespace(cur_ch) {
        // Punctuation
        while idx < len && !is_vim_word_char(chars[idx].1) && !is_vim_whitespace(chars[idx].1) {
            idx += 1;
        }
    }

    // Skip whitespace (but not newlines for multi-line jumps in some contexts -
    // standard vim `w` crosses lines)
    while idx < len && is_vim_whitespace(chars[idx].1) {
        idx += 1;
    }

    if idx < len {
        chars[idx].0
    } else {
        text.len()
    }
}

/// Move to the start of the previous vim word.
pub fn prev_vim_word(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() || offset == 0 {
        return 0;
    }

    // Find char index just before offset
    let mut idx = match chars.iter().rposition(|&(b, _)| b < offset) {
        Some(i) => i,
        None => return 0,
    };

    // Skip whitespace backward
    while idx > 0 && is_vim_whitespace(chars[idx].1) {
        idx -= 1;
    }

    if is_vim_word_char(chars[idx].1) {
        while idx > 0 && is_vim_word_char(chars[idx - 1].1) {
            idx -= 1;
        }
    } else if !is_vim_whitespace(chars[idx].1) {
        while idx > 0
            && !is_vim_word_char(chars[idx - 1].1)
            && !is_vim_whitespace(chars[idx - 1].1)
        {
            idx -= 1;
        }
    }

    chars[idx].0
}

/// Move to the end of the current/next vim word.
pub fn end_of_vim_word(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();
    if len == 0 {
        return 0;
    }

    let mut idx = match chars.iter().position(|&(b, _)| b >= offset) {
        Some(i) => i,
        None => return offset,
    };

    // Move at least one position forward
    if idx + 1 < len {
        idx += 1;
    } else {
        return chars[idx].0;
    }

    // Skip whitespace
    while idx < len && is_vim_whitespace(chars[idx].1) {
        idx += 1;
    }

    if idx >= len {
        return chars[len - 1].0;
    }

    // Find end of this word
    if is_vim_word_char(chars[idx].1) {
        while idx + 1 < len && is_vim_word_char(chars[idx + 1].1) {
            idx += 1;
        }
    } else if !is_vim_whitespace(chars[idx].1) {
        while idx + 1 < len
            && !is_vim_word_char(chars[idx + 1].1)
            && !is_vim_whitespace(chars[idx + 1].1)
        {
            idx += 1;
        }
    }

    chars[idx].0
}

/// Move to the start of the next WORD (whitespace-delimited).
pub fn next_word_big(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();

    let mut idx = match chars.iter().position(|&(b, _)| b >= offset) {
        Some(i) => i,
        None => return offset,
    };

    // Skip non-whitespace
    while idx < len && !is_vim_whitespace(chars[idx].1) {
        idx += 1;
    }
    // Skip whitespace
    while idx < len && is_vim_whitespace(chars[idx].1) {
        idx += 1;
    }

    if idx < len {
        chars[idx].0
    } else {
        text.len()
    }
}

/// Move to the start of the previous WORD.
pub fn prev_word_big(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() || offset == 0 {
        return 0;
    }

    let mut idx = match chars.iter().rposition(|&(b, _)| b < offset) {
        Some(i) => i,
        None => return 0,
    };

    // Skip whitespace backward
    while idx > 0 && is_vim_whitespace(chars[idx].1) {
        idx -= 1;
    }

    // Skip non-whitespace backward
    while idx > 0 && !is_vim_whitespace(chars[idx - 1].1) {
        idx -= 1;
    }

    chars[idx].0
}

/// Move to the end of the current/next WORD.
pub fn end_of_word_big(text: &str, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();
    if len == 0 {
        return 0;
    }

    let mut idx = match chars.iter().position(|&(b, _)| b >= offset) {
        Some(i) => i,
        None => return offset,
    };

    if idx + 1 < len {
        idx += 1;
    } else {
        return chars[idx].0;
    }

    // Skip whitespace
    while idx < len && is_vim_whitespace(chars[idx].1) {
        idx += 1;
    }

    if idx >= len {
        return chars[len - 1].0;
    }

    // Find end of WORD
    while idx + 1 < len && !is_vim_whitespace(chars[idx + 1].1) {
        idx += 1;
    }

    chars[idx].0
}

// ============================================================================
// Find character motions
// ============================================================================

/// Find a character on the current line, returning the offset if found.
/// Supports f (forward to), F (backward to), t (forward till), T (backward till).
pub fn find_character(
    text: &str,
    offset: usize,
    ch: char,
    find_type: FindType,
    count: usize,
) -> Option<usize> {
    match find_type {
        FindType::F => find_char_forward(text, offset, ch, count, false),
        FindType::FBack => find_char_backward(text, offset, ch, count, false),
        FindType::T => find_char_forward(text, offset, ch, count, true),
        FindType::TBack => find_char_backward(text, offset, ch, count, true),
    }
}

fn find_char_forward(
    text: &str,
    offset: usize,
    target: char,
    count: usize,
    till: bool,
) -> Option<usize> {
    let line_end = {
        let rest = &text[offset..];
        match rest.find('\n') {
            Some(pos) => offset + pos,
            None => text.len(),
        }
    };

    let search_start = next_char_offset(text, offset).unwrap_or(text.len());
    let search_text = &text[search_start..line_end];

    let mut found = 0;
    let mut last_pos = None;
    for (i, ch) in search_text.char_indices() {
        if ch == target {
            found += 1;
            last_pos = Some(search_start + i);
            if found == count {
                break;
            }
        }
    }

    if found < count {
        return None;
    }

    let pos = last_pos?;
    if till {
        // t: one before the found character
        Some(prev_char_boundary(text, pos))
    } else {
        Some(pos)
    }
}

fn find_char_backward(
    text: &str,
    offset: usize,
    target: char,
    count: usize,
    till: bool,
) -> Option<usize> {
    let line_start = start_of_logical_line(text, offset);
    let search_text = &text[line_start..offset];

    let mut positions: Vec<usize> = Vec::new();
    for (i, ch) in search_text.char_indices() {
        if ch == target {
            positions.push(line_start + i);
        }
    }

    if positions.len() < count {
        return None;
    }

    let pos = positions[positions.len() - count];
    if till {
        // T: one after the found character
        next_char_offset(text, pos)
    } else {
        Some(pos)
    }
}

/// Check if offset is at the end of text (last char position or empty).
pub fn is_at_end(text: &str, offset: usize) -> bool {
    if text.is_empty() {
        return true;
    }
    // At or past the last character
    let last_char_start = prev_char_boundary(text, text.len());
    offset >= last_char_start
}

/// Get the next grapheme/char offset (one past current char).
pub fn next_offset(text: &str, offset: usize) -> usize {
    next_char_offset(text, offset).unwrap_or(text.len())
}

/// Count occurrences of a character in a string.
pub fn count_char(text: &str, ch: char) -> usize {
    text.chars().filter(|&c| c == ch).count()
}

/// Get the current line number (0-based) for an offset.
pub fn line_of_offset(text: &str, offset: usize) -> usize {
    count_char(&text[..offset.min(text.len())], '\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_left_right() {
        let text = "hello";
        assert_eq!(move_left(text, 0), 0);
        assert_eq!(move_left(text, 1), 0);
        assert_eq!(move_right(text, 0), 1);
        assert_eq!(move_right(text, 4), 4); // last char, can't move right
    }

    #[test]
    fn test_line_start_end() {
        let text = "hello\nworld";
        assert_eq!(start_of_logical_line(text, 7), 6);
        assert_eq!(end_of_logical_line(text, 7), 10);
        assert_eq!(start_of_logical_line(text, 3), 0);
        assert_eq!(end_of_logical_line(text, 3), 4);
    }

    #[test]
    fn test_word_forward() {
        let text = "hello world foo";
        assert_eq!(next_vim_word(text, 0), 6);
        assert_eq!(next_vim_word(text, 6), 12);
    }

    #[test]
    fn test_word_backward() {
        let text = "hello world";
        assert_eq!(prev_vim_word(text, 8), 6);
        assert_eq!(prev_vim_word(text, 6), 0);
    }

    #[test]
    fn test_find_char() {
        let text = "hello world";
        assert_eq!(
            find_character(text, 0, 'o', FindType::F, 1),
            Some(4)
        );
        assert_eq!(
            find_character(text, 0, 'o', FindType::T, 1),
            Some(3)
        );
    }

    #[test]
    fn test_up_down() {
        let text = "abc\ndefgh\nij";
        // From 'b' (offset 1), go down -> 'd' line col 1 -> offset 5
        assert_eq!(move_down_logical(text, 1), 5);
        // From offset 5 ('e'), go up -> back to offset 1
        assert_eq!(move_up_logical(text, 5), 1);
    }

    #[test]
    fn test_resolve_motion_with_count() {
        let text = "one two three four";
        // 2w from start should jump two words
        let result = resolve_motion(text, 0, "w", 2);
        assert_eq!(result, 8); // "three"
    }
}
