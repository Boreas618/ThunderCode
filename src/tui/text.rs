//! Text wrapping, truncation, and measurement.
//!
//! Provides Unicode-aware text width computation and several wrapping modes
//! matching the ref implementation's `wrap-text.ts` and `measure-text.ts`.

use unicode_width::UnicodeWidthStr;
use unicode_width::UnicodeWidthChar;

/// Text wrap mode. Matches the ref `Styles['textWrap']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    /// Soft-wrap at `max_width`, preserving trailing whitespace.
    #[default]
    Wrap,
    /// Soft-wrap at `max_width`, trimming trailing whitespace.
    WrapTrim,
    /// Truncate at end with ellipsis.
    #[allow(dead_code)]
    End,
    /// Truncate in the middle with ellipsis.
    #[allow(dead_code)]
    Middle,
    /// Alias for truncate-end.
    Truncate,
    /// Truncate at start with ellipsis.
    TruncateStart,
    /// Truncate in the middle with ellipsis.
    TruncateMiddle,
}

impl TextWrap {
    /// Parse from the ref's string representation.
    pub fn from_str_ref(s: &str) -> Self {
        match s {
            "wrap" => Self::Wrap,
            "wrap-trim" => Self::WrapTrim,
            "end" => Self::End,
            "middle" => Self::Middle,
            "truncate" | "truncate-end" => Self::Truncate,
            "truncate-start" => Self::TruncateStart,
            "truncate-middle" => Self::TruncateMiddle,
            _ => Self::Wrap,
        }
    }
}

const ELLIPSIS: &str = "\u{2026}";

/// Compute the display width of a string, accounting for Unicode
/// double-width characters (CJK, certain emoji, etc.).
pub fn string_width(s: &str) -> usize {
    // Filter out ANSI escape sequences for width computation
    strip_ansi_width(s)
}

/// String width ignoring ANSI escape sequences.
fn strip_ansi_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut in_escape = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if in_escape {
            // Inside ESC sequence, skip until final byte
            if c == '[' {
                // CSI sequence: skip until 0x40-0x7E
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if (0x40..=0x7E).contains(&(next as u32)) {
                        break;
                    }
                }
                in_escape = false;
            } else if c == ']' {
                // OSC sequence: skip until BEL or ST
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
                in_escape = false;
            } else {
                // Simple ESC sequence (e.g., ESC D)
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            width += UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    width
}

/// Slice a string by display column positions, respecting ANSI codes.
/// Returns the substring from display column `start` to `end` (exclusive).
fn slice_by_width(text: &str, start: usize, end: usize) -> String {
    let mut result = String::new();
    let mut col = 0usize;
    let mut in_escape = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if in_escape {
            // Always include escape sequence chars
            result.push(c);
            if c == '[' {
                while let Some(&next) = chars.peek() {
                    result.push(next);
                    chars.next();
                    if (0x40..=0x7E).contains(&(next as u32)) {
                        break;
                    }
                }
            } else if c == ']' {
                while let Some(&next) = chars.peek() {
                    result.push(next);
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' {
                        result.push('\x1b');
                        if chars.peek() == Some(&'\\') {
                            result.push('\\');
                            chars.next();
                        }
                        break;
                    }
                }
            }
            in_escape = false;
            continue;
        }

        if c == '\x1b' {
            in_escape = true;
            if col >= start && col < end {
                result.push(c);
            }
            continue;
        }

        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if col >= start && col + w <= end {
            result.push(c);
        }
        col += w;
        if col >= end {
            break;
        }
    }
    result
}

/// Truncate a single line of text to fit within `columns` display columns,
/// placing an ellipsis at the specified position.
fn truncate_text(text: &str, columns: usize, position: TruncatePosition) -> String {
    if columns < 1 {
        return String::new();
    }
    if columns == 1 {
        return ELLIPSIS.into();
    }

    let length = string_width(text);
    if length <= columns {
        return text.into();
    }

    match position {
        TruncatePosition::Start => {
            format!(
                "{}{}",
                ELLIPSIS,
                slice_by_width(text, length - columns + 1, length)
            )
        }
        TruncatePosition::Middle => {
            let half = columns / 2;
            format!(
                "{}{}{}",
                slice_by_width(text, 0, half),
                ELLIPSIS,
                slice_by_width(text, length - (columns - half) + 1, length)
            )
        }
        TruncatePosition::End => {
            format!(
                "{}{}",
                slice_by_width(text, 0, columns - 1),
                ELLIPSIS
            )
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TruncatePosition {
    Start,
    Middle,
    End,
}

/// Wrap text according to the specified wrap type.
pub fn wrap_text(text: &str, max_width: usize, wrap_type: TextWrap) -> String {
    match wrap_type {
        TextWrap::Wrap => hard_wrap(text, max_width, false),
        TextWrap::WrapTrim => hard_wrap(text, max_width, true),
        TextWrap::Truncate | TextWrap::End => {
            truncate_text(text, max_width, TruncatePosition::End)
        }
        TextWrap::TruncateStart => truncate_text(text, max_width, TruncatePosition::Start),
        TextWrap::TruncateMiddle | TextWrap::Middle => {
            truncate_text(text, max_width, TruncatePosition::Middle)
        }
    }
}

/// Hard-wrap text at `max_width` display columns.
/// Each source line (split by `\n`) is independently wrapped.
fn hard_wrap(text: &str, max_width: usize, trim: bool) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut result = String::new();
    let mut first_line = true;

    for line in text.split('\n') {
        if !first_line {
            result.push('\n');
        }
        first_line = false;

        let w = string_width(line);
        if w <= max_width {
            if trim {
                result.push_str(line.trim_end());
            } else {
                result.push_str(line);
            }
            continue;
        }

        // Need to wrap this line
        let mut col = 0usize;
        let mut line_start = true;
        for c in line.chars() {
            let cw = UnicodeWidthChar::width(c).unwrap_or(0);
            if col + cw > max_width && col > 0 {
                if trim {
                    // Trim trailing whitespace before the wrap point
                    while result.ends_with(' ') {
                        result.pop();
                    }
                }
                result.push('\n');
                col = 0;
                line_start = true;
            }
            if trim && line_start && c == ' ' {
                // Skip leading whitespace after wrap in trim mode? No, preserve it.
            }
            result.push(c);
            col += cw;
            if cw > 0 {
                line_start = false;
            }
        }
    }
    result
}

/// Measure text dimensions: (width, height) in display columns and lines.
///
/// Width is the widest line. Height accounts for wrapping: each source line
/// contributes `ceil(line_width / max_width)` visual lines (or 1 if empty).
pub fn measure_text(text: &str, max_width: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }

    let no_wrap = max_width == 0 || max_width == usize::MAX;
    let mut height = 0usize;
    let mut width = 0usize;

    for line in text.split('\n') {
        let w = string_width(line);
        width = width.max(w);
        if no_wrap {
            height += 1;
        } else if w == 0 {
            height += 1;
        } else {
            height += (w + max_width - 1) / max_width; // ceil division
        }
    }

    (width, height)
}

/// Compute the display width of a single line (no newlines).
/// This is a convenience wrapper around `string_width`.
pub fn line_width(line: &str) -> usize {
    // Use UnicodeWidthStr for plain text, strip_ansi_width for text with ANSI
    if line.contains('\x1b') {
        strip_ansi_width(line)
    } else {
        UnicodeWidthStr::width(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_width_ascii() {
        assert_eq!(string_width("hello"), 5);
        assert_eq!(string_width(""), 0);
        assert_eq!(string_width("a b c"), 5);
    }

    #[test]
    fn test_string_width_cjk() {
        // CJK characters are double-width
        assert_eq!(string_width("\u{4e16}\u{754c}"), 4); // "world" in Chinese
    }

    #[test]
    fn test_string_width_with_ansi() {
        // ANSI codes should not contribute to width
        assert_eq!(string_width("\x1b[31mhello\x1b[0m"), 5);
        assert_eq!(string_width("\x1b[1;32mtest\x1b[0m"), 4);
    }

    #[test]
    fn test_measure_text_single_line() {
        assert_eq!(measure_text("hello", 80), (5, 1));
    }

    #[test]
    fn test_measure_text_multiline() {
        assert_eq!(measure_text("hello\nworld", 80), (5, 2));
    }

    #[test]
    fn test_measure_text_wrapping() {
        // "hello world" is 11 chars, with max_width=5, each line wraps
        assert_eq!(measure_text("hello world", 5), (11, 3));
    }

    #[test]
    fn test_measure_text_empty() {
        assert_eq!(measure_text("", 80), (0, 0));
    }

    #[test]
    fn test_measure_text_empty_lines() {
        assert_eq!(measure_text("\n\n", 80), (0, 3));
    }

    #[test]
    fn test_wrap_text_no_wrap_needed() {
        assert_eq!(wrap_text("hi", 10, TextWrap::Wrap), "hi");
    }

    #[test]
    fn test_wrap_text_hard_wrap() {
        let result = wrap_text("abcdef", 3, TextWrap::Wrap);
        assert_eq!(result, "abc\ndef");
    }

    #[test]
    fn test_truncate_end() {
        let result = wrap_text("hello world", 7, TextWrap::Truncate);
        assert_eq!(string_width(&result), 7);
        assert!(result.ends_with(ELLIPSIS));
        assert!(result.starts_with("hello "));
    }

    #[test]
    fn test_truncate_start() {
        let result = wrap_text("hello world", 7, TextWrap::TruncateStart);
        assert_eq!(string_width(&result), 7);
        assert!(result.starts_with(ELLIPSIS));
    }

    #[test]
    fn test_truncate_middle() {
        let result = wrap_text("hello world", 7, TextWrap::TruncateMiddle);
        assert_eq!(string_width(&result), 7);
        assert!(result.contains(ELLIPSIS));
    }

    #[test]
    fn test_truncate_short_enough() {
        assert_eq!(wrap_text("hi", 10, TextWrap::Truncate), "hi");
    }

    #[test]
    fn test_truncate_columns_1() {
        assert_eq!(wrap_text("hello", 1, TextWrap::Truncate), ELLIPSIS);
    }

    #[test]
    fn test_truncate_columns_0() {
        assert_eq!(wrap_text("hello", 0, TextWrap::Truncate), "");
    }

    #[test]
    fn test_text_wrap_from_str_ref() {
        assert_eq!(TextWrap::from_str_ref("wrap"), TextWrap::Wrap);
        assert_eq!(TextWrap::from_str_ref("wrap-trim"), TextWrap::WrapTrim);
        assert_eq!(TextWrap::from_str_ref("truncate"), TextWrap::Truncate);
        assert_eq!(
            TextWrap::from_str_ref("truncate-start"),
            TextWrap::TruncateStart
        );
        assert_eq!(
            TextWrap::from_str_ref("truncate-middle"),
            TextWrap::TruncateMiddle
        );
        assert_eq!(TextWrap::from_str_ref("unknown"), TextWrap::Wrap);
    }

    #[test]
    fn test_line_width() {
        assert_eq!(line_width("hello"), 5);
        assert_eq!(line_width("\x1b[31mhello\x1b[0m"), 5);
    }
}
