//! Structured diff rendering.
//!
//! Parses unified diff text and produces styled lines with:
//! - Green/red line coloring for additions/removals
//! - Line numbers in the gutter
//! - File path headers
//! - Word-level change highlighting within modified lines

use crate::tui::markdown::{SpanStyle, StyledLine, StyledSpan};
use crate::tui::style::{Color, NamedColor};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a unified diff string into styled lines.
pub fn render_diff(diff_text: &str) -> Vec<StyledLine> {
    let mut lines = Vec::new();
    let mut old_lineno: Option<usize> = None;
    let mut new_lineno: Option<usize> = None;

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("diff ") {
            // diff header
            lines.push(styled_diff_header(raw_line));
        } else if raw_line.starts_with("--- ") || raw_line.starts_with("+++ ") {
            // file path line
            lines.push(styled_file_path(raw_line));
        } else if raw_line.starts_with("@@ ") {
            // Hunk header: parse line numbers
            let (old, new) = parse_hunk_header(raw_line);
            old_lineno = Some(old);
            new_lineno = Some(new);
            lines.push(styled_hunk_header(raw_line));
        } else if let Some(rest) = raw_line.strip_prefix('+') {
            // Added line
            let lineno = new_lineno.unwrap_or(0);
            lines.push(styled_added_line(rest, lineno));
            if let Some(ref mut n) = new_lineno {
                *n += 1;
            }
        } else if let Some(rest) = raw_line.strip_prefix('-') {
            // Removed line
            let lineno = old_lineno.unwrap_or(0);
            lines.push(styled_removed_line(rest, lineno));
            if let Some(ref mut n) = old_lineno {
                *n += 1;
            }
        } else if raw_line.starts_with(' ') {
            // Context line
            let lineno = new_lineno.unwrap_or(0);
            lines.push(styled_context_line(&raw_line[1..], lineno));
            if let Some(ref mut n) = old_lineno {
                *n += 1;
            }
            if let Some(ref mut n) = new_lineno {
                *n += 1;
            }
        } else {
            // Other (e.g., "\ No newline at end of file")
            let mut sl = StyledLine::new();
            sl.push(
                raw_line,
                SpanStyle {
                    dim: true,
                    ..SpanStyle::default()
                },
            );
            lines.push(sl);
        }
    }

    lines
}

// ---------------------------------------------------------------------------
// Line formatters
// ---------------------------------------------------------------------------

fn gutter(lineno: usize) -> String {
    format!("{:>4} ", lineno)
}

fn styled_diff_header(line: &str) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        line,
        SpanStyle {
            bold: true,
            ..SpanStyle::default()
        },
    );
    sl
}

fn styled_file_path(line: &str) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        line,
        SpanStyle {
            bold: true,
            fg: Some(Color::Named(NamedColor::White)),
            ..SpanStyle::default()
        },
    );
    sl
}

fn styled_hunk_header(line: &str) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        line,
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Cyan)),
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl
}

fn styled_added_line(content: &str, lineno: usize) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        gutter(lineno),
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Green)),
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        "+",
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Green)),
            bold: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        content,
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Green)),
            ..SpanStyle::default()
        },
    );
    sl
}

fn styled_removed_line(content: &str, lineno: usize) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        gutter(lineno),
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Red)),
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        "-",
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Red)),
            bold: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        content,
        SpanStyle {
            fg: Some(Color::Named(NamedColor::Red)),
            ..SpanStyle::default()
        },
    );
    sl
}

fn styled_context_line(content: &str, lineno: usize) -> StyledLine {
    let mut sl = StyledLine::new();
    sl.push(
        gutter(lineno),
        SpanStyle {
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        " ",
        SpanStyle {
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl.push(
        content,
        SpanStyle {
            dim: true,
            ..SpanStyle::default()
        },
    );
    sl
}

// ---------------------------------------------------------------------------
// Word-level diff within a changed line pair
// ---------------------------------------------------------------------------

/// Given an old line and new line, produce spans with word-level highlighting.
///
/// Words that differ get brighter coloring (bold + background tint).
pub fn word_diff_spans(
    old_text: &str,
    new_text: &str,
) -> (Vec<StyledSpan>, Vec<StyledSpan>) {
    let old_words: Vec<&str> = old_text.split_inclusive(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .collect();
    let new_words: Vec<&str> = new_text.split_inclusive(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .collect();

    let mut old_spans = Vec::new();
    let mut new_spans = Vec::new();

    // Simple LCS-based word diff (greedy approach for performance)
    let max_len = old_words.len().max(new_words.len());
    let mut oi = 0;
    let mut ni = 0;

    while oi < old_words.len() || ni < new_words.len() {
        if oi < old_words.len() && ni < new_words.len() && old_words[oi] == new_words[ni] {
            // Matching word
            old_spans.push(StyledSpan {
                text: old_words[oi].to_string(),
                style: SpanStyle {
                    fg: Some(Color::Named(NamedColor::Red)),
                    dim: true,
                    ..SpanStyle::default()
                },
            });
            new_spans.push(StyledSpan {
                text: new_words[ni].to_string(),
                style: SpanStyle {
                    fg: Some(Color::Named(NamedColor::Green)),
                    dim: true,
                    ..SpanStyle::default()
                },
            });
            oi += 1;
            ni += 1;
        } else {
            // Different word(s) -- highlight them
            if oi < old_words.len() {
                old_spans.push(StyledSpan {
                    text: old_words[oi].to_string(),
                    style: SpanStyle {
                        fg: Some(Color::Named(NamedColor::Red)),
                        bg: Some(Color::Ansi256(52)), // dark red bg
                        bold: true,
                        ..SpanStyle::default()
                    },
                });
                oi += 1;
            }
            if ni < new_words.len() {
                new_spans.push(StyledSpan {
                    text: new_words[ni].to_string(),
                    style: SpanStyle {
                        fg: Some(Color::Named(NamedColor::Green)),
                        bg: Some(Color::Ansi256(22)), // dark green bg
                        bold: true,
                        ..SpanStyle::default()
                    },
                });
                ni += 1;
            }
        }

        // Safety: avoid infinite loop on degenerate input
        if oi + ni > max_len * 3 {
            break;
        }
    }

    (old_spans, new_spans)
}

// ---------------------------------------------------------------------------
// Hunk header parser
// ---------------------------------------------------------------------------

/// Parse `@@ -OLD_START,OLD_COUNT +NEW_START,NEW_COUNT @@` into (old_start, new_start).
fn parse_hunk_header(line: &str) -> (usize, usize) {
    // Format: @@ -START[,COUNT] +START[,COUNT] @@
    let mut old_start = 1usize;
    let mut new_start = 1usize;

    if let Some(rest) = line.strip_prefix("@@ ") {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            // Parse old range "-N,M" or "-N"
            if let Some(old_range) = parts[0].strip_prefix('-') {
                if let Some(comma) = old_range.find(',') {
                    old_start = old_range[..comma].parse().unwrap_or(1);
                } else {
                    old_start = old_range.parse().unwrap_or(1);
                }
            }
            // Parse new range "+N,M" or "+N"
            if let Some(new_range) = parts[1].strip_prefix('+') {
                if let Some(comma) = new_range.find(',') {
                    new_start = new_range[..comma].parse().unwrap_or(1);
                } else {
                    new_start = new_range.parse().unwrap_or(1);
                }
            }
        }
    }

    (old_start, new_start)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -1,5 +1,7 @@"), (1, 1));
        assert_eq!(parse_hunk_header("@@ -10,3 +12,5 @@ fn main"), (10, 12));
        assert_eq!(parse_hunk_header("@@ -1 +1 @@"), (1, 1));
    }

    #[test]
    fn test_render_diff_basic() {
        let diff = "\
diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 line1
-old line
+new line
+added line
 line3";

        let lines = render_diff(diff);
        assert!(lines.len() >= 7);

        // Check that added lines are green
        let added = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.fg == Some(Color::Named(NamedColor::Green)));
        assert!(added);

        // Check that removed lines are red
        let removed = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.fg == Some(Color::Named(NamedColor::Red)));
        assert!(removed);
    }

    #[test]
    fn test_render_diff_empty() {
        let lines = render_diff("");
        assert!(lines.is_empty());
    }

    #[test]
    fn test_word_diff() {
        let (old_spans, new_spans) = word_diff_spans(
            "hello world",
            "hello universe",
        );
        assert!(!old_spans.is_empty());
        assert!(!new_spans.is_empty());
    }

    #[test]
    fn test_gutter_format() {
        assert_eq!(gutter(1), "   1 ");
        assert_eq!(gutter(999), " 999 ");
    }
}
