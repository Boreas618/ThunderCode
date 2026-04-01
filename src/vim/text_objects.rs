//! Vim text object finding.
//!
//! Functions for finding text object boundaries (iw, aw, i", a(, etc.)

/// A range in the text selected by a text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjectRange {
    pub start: usize,
    pub end: usize,
}

/// Delimiter pairs for bracket/quote text objects.
fn get_pair(obj_type: char) -> Option<(char, char)> {
    match obj_type {
        '(' | ')' | 'b' => Some(('(', ')')),
        '[' | ']' => Some(('[', ']')),
        '{' | '}' | 'B' => Some(('{', '}')),
        '<' | '>' => Some(('<', '>')),
        '"' => Some(('"', '"')),
        '\'' => Some(('\'', '\'')),
        '`' => Some(('`', '`')),
        _ => None,
    }
}

/// Find a text object at the given position.
pub fn find_text_object(
    text: &str,
    offset: usize,
    obj_type: char,
    is_inner: bool,
) -> Option<TextObjectRange> {
    match obj_type {
        'w' => find_word_object(text, offset, is_inner, is_vim_word_char),
        'W' => find_word_object(text, offset, is_inner, |ch| !is_vim_whitespace(ch)),
        _ => {
            let (open, close) = get_pair(obj_type)?;
            if open == close {
                find_quote_object(text, offset, open, is_inner)
            } else {
                find_bracket_object(text, offset, open, close, is_inner)
            }
        }
    }
}

// ============================================================================
// Character classification
// ============================================================================

/// Check if a character is a vim word character (alphanumeric or underscore).
pub fn is_vim_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Check if a character is vim whitespace.
pub fn is_vim_whitespace(ch: char) -> bool {
    ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r'
}

/// Check if a character is vim punctuation (not word char and not whitespace).
pub fn is_vim_punctuation(ch: char) -> bool {
    !is_vim_word_char(ch) && !is_vim_whitespace(ch)
}

// ============================================================================
// Word objects
// ============================================================================

fn find_word_object(
    text: &str,
    offset: usize,
    is_inner: bool,
    is_word_char: fn(char) -> bool,
) -> Option<TextObjectRange> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Find which char index the byte offset falls in
    let mut char_idx = 0;
    {
        let mut byte_pos = 0;
        for (i, &ch) in chars.iter().enumerate() {
            if byte_pos + ch.len_utf8() > offset {
                char_idx = i;
                break;
            }
            byte_pos += ch.len_utf8();
            if i == chars.len() - 1 {
                char_idx = i;
            }
        }
    }

    let is_ws = |idx: usize| -> bool {
        chars.get(idx).map_or(false, |&ch| is_vim_whitespace(ch))
    };
    let is_word = |idx: usize| -> bool { chars.get(idx).map_or(false, |&ch| is_word_char(ch)) };
    let is_punct =
        |idx: usize| -> bool { chars.get(idx).map_or(false, |&ch| is_vim_punctuation(ch)) };

    let char_offset = |idx: usize| -> usize {
        chars.iter().take(idx).map(|ch| ch.len_utf8()).sum()
    };

    let mut start_idx = char_idx;
    let mut end_idx = char_idx;

    if is_word(char_idx) {
        while start_idx > 0 && is_word(start_idx - 1) {
            start_idx -= 1;
        }
        while end_idx < chars.len() && is_word(end_idx) {
            end_idx += 1;
        }
    } else if is_ws(char_idx) {
        while start_idx > 0 && is_ws(start_idx - 1) {
            start_idx -= 1;
        }
        while end_idx < chars.len() && is_ws(end_idx) {
            end_idx += 1;
        }
        return Some(TextObjectRange {
            start: char_offset(start_idx),
            end: char_offset(end_idx),
        });
    } else if is_punct(char_idx) {
        while start_idx > 0 && is_punct(start_idx - 1) {
            start_idx -= 1;
        }
        while end_idx < chars.len() && is_punct(end_idx) {
            end_idx += 1;
        }
    } else {
        return None;
    }

    if !is_inner {
        // Include surrounding whitespace
        if end_idx < chars.len() && is_ws(end_idx) {
            while end_idx < chars.len() && is_ws(end_idx) {
                end_idx += 1;
            }
        } else if start_idx > 0 && is_ws(start_idx - 1) {
            while start_idx > 0 && is_ws(start_idx - 1) {
                start_idx -= 1;
            }
        }
    }

    Some(TextObjectRange {
        start: char_offset(start_idx),
        end: char_offset(end_idx),
    })
}

// ============================================================================
// Quote objects
// ============================================================================

fn find_quote_object(
    text: &str,
    offset: usize,
    quote: char,
    is_inner: bool,
) -> Option<TextObjectRange> {
    // Work on the current line
    let line_start = text[..offset]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let line_end = text[offset..]
        .find('\n')
        .map(|p| offset + p)
        .unwrap_or(text.len());
    let line = &text[line_start..line_end];
    let pos_in_line = offset - line_start;

    // Collect positions of the quote character in the line
    let positions: Vec<usize> = line
        .char_indices()
        .filter(|&(_, ch)| ch == quote)
        .map(|(i, _)| i)
        .collect();

    // Pair quotes: 0-1, 2-3, 4-5, etc.
    let mut i = 0;
    while i + 1 < positions.len() {
        let qs = positions[i];
        let qe = positions[i + 1];
        if qs <= pos_in_line && pos_in_line <= qe {
            return if is_inner {
                Some(TextObjectRange {
                    start: line_start + qs + quote.len_utf8(),
                    end: line_start + qe,
                })
            } else {
                Some(TextObjectRange {
                    start: line_start + qs,
                    end: line_start + qe + quote.len_utf8(),
                })
            };
        }
        i += 2;
    }

    None
}

// ============================================================================
// Bracket objects
// ============================================================================

fn find_bracket_object(
    text: &str,
    offset: usize,
    open: char,
    close: char,
    is_inner: bool,
) -> Option<TextObjectRange> {
    let bytes = text.as_bytes();

    // Search backward for matching open bracket
    let mut depth: i32 = 0;
    let mut start: Option<usize> = None;
    let mut i = offset;
    loop {
        let ch = bytes[i] as char;
        if ch == close && i != offset {
            depth += 1;
        } else if ch == open {
            if depth == 0 {
                start = Some(i);
                break;
            }
            depth -= 1;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    let start = start?;

    // Search forward for matching close bracket
    depth = 0;
    let mut end: Option<usize> = None;
    for j in (start + 1)..text.len() {
        let ch = bytes[j] as char;
        if ch == open {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                end = Some(j);
                break;
            }
            depth -= 1;
        }
    }
    let end = end?;

    if is_inner {
        Some(TextObjectRange {
            start: start + open.len_utf8(),
            end,
        })
    } else {
        Some(TextObjectRange {
            start,
            end: end + close.len_utf8(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_word() {
        let text = "hello world";
        let range = find_text_object(text, 1, 'w', true).unwrap();
        assert_eq!(&text[range.start..range.end], "hello");
    }

    #[test]
    fn test_a_word() {
        let text = "hello world";
        let range = find_text_object(text, 1, 'w', false).unwrap();
        // "a word" includes trailing space
        assert_eq!(&text[range.start..range.end], "hello ");
    }

    #[test]
    fn test_inner_quotes() {
        let text = r#"say "hello" please"#;
        let range = find_text_object(text, 6, '"', true).unwrap();
        assert_eq!(&text[range.start..range.end], "hello");
    }

    #[test]
    fn test_around_quotes() {
        let text = r#"say "hello" please"#;
        let range = find_text_object(text, 6, '"', false).unwrap();
        assert_eq!(&text[range.start..range.end], "\"hello\"");
    }

    #[test]
    fn test_inner_parens() {
        let text = "foo(bar, baz)end";
        let range = find_text_object(text, 5, '(', true).unwrap();
        assert_eq!(&text[range.start..range.end], "bar, baz");
    }

    #[test]
    fn test_around_parens() {
        let text = "foo(bar, baz)end";
        let range = find_text_object(text, 5, ')', false).unwrap();
        assert_eq!(&text[range.start..range.end], "(bar, baz)");
    }

    #[test]
    fn test_inner_braces() {
        let text = "if { x + 1 }";
        let range = find_text_object(text, 6, '{', true).unwrap();
        assert_eq!(&text[range.start..range.end], " x + 1 ");
    }

    #[test]
    fn test_nested_brackets() {
        let text = "[a[b]c]";
        // cursor on 'b' (offset 3) should find inner [b]
        let range = find_text_object(text, 3, '[', true).unwrap();
        assert_eq!(&text[range.start..range.end], "b");
    }

    #[test]
    fn test_no_match() {
        let text = "no parens here";
        assert!(find_text_object(text, 3, '(', true).is_none());
    }

    #[test]
    fn test_big_word() {
        let text = "hello-world foo";
        let range = find_text_object(text, 2, 'W', true).unwrap();
        assert_eq!(&text[range.start..range.end], "hello-world");
    }
}
