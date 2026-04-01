//! Syntax highlighting for code blocks using `syntect`.
//!
//! Maps source code + language to [`StyledLine`]s with per-token foreground
//! colors derived from syntect's theme system.

use std::sync::OnceLock;

use syntect::highlighting::{
    FontStyle, Style as SyntectStyle, ThemeSet,
};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

use crate::tui::markdown::{SpanStyle, StyledLine};
use crate::tui::style::Color;

// ---------------------------------------------------------------------------
// Global syntax resources (loaded once)
// ---------------------------------------------------------------------------

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// The default theme name used for terminal rendering.
const DEFAULT_THEME: &str = "base16-ocean.dark";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Highlight a code block, returning styled lines.
///
/// `language` is the fenced-code-block language tag (e.g. `"rust"`, `"py"`).
/// Falls back to plain text if the language is not recognized.
/// `_width` is reserved for future line-wrapping support.
pub fn highlight_code(code: &str, language: &str, _width: usize) -> Vec<StyledLine> {
    let ss = syntax_set();
    let ts = theme_set();

    // Look up the syntax definition
    let syntax = if language.is_empty() {
        ss.find_syntax_plain_text()
    } else {
        ss.find_syntax_by_token(language)
            .or_else(|| ss.find_syntax_by_extension(language))
            .unwrap_or_else(|| ss.find_syntax_plain_text())
    };

    let theme = match ts.themes.get(DEFAULT_THEME) {
        Some(t) => t,
        None => {
            // Fallback: return plain text lines with code background
            return plain_code_lines(code);
        }
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut result = Vec::new();

    for line_text in code.lines() {
        match highlighter.highlight_line(line_text, ss) {
            Ok(ranges) => {
                let mut styled_line = StyledLine::new();
                // Left margin for code block
                styled_line.push(
                    "  ",
                    SpanStyle {
                        bg: Some(Color::Ansi256(235)),
                        ..SpanStyle::default()
                    },
                );
                for (style, text) in ranges {
                    styled_line.push(
                        text.to_string(),
                        syntect_to_span_style(style),
                    );
                }
                result.push(styled_line);
            }
            Err(_) => {
                // Highlight failure: emit as plain
                let mut styled_line = StyledLine::new();
                styled_line.push(
                    format!("  {}", line_text),
                    SpanStyle {
                        bg: Some(Color::Ansi256(235)),
                        ..SpanStyle::default()
                    },
                );
                result.push(styled_line);
            }
        }
    }

    // If code was empty or ended with newline, don't add extra empty line
    if result.is_empty() {
        result.push(StyledLine::new());
    }

    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a syntect style into our `SpanStyle`.
fn syntect_to_span_style(style: SyntectStyle) -> SpanStyle {
    let fg = Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    );
    let bg = Color::Ansi256(235); // consistent dark background for code

    SpanStyle {
        fg: Some(fg),
        bg: Some(bg),
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
        ..SpanStyle::default()
    }
}

/// Fallback: render code as plain monospace lines with dark background.
fn plain_code_lines(code: &str) -> Vec<StyledLine> {
    code.lines()
        .map(|line| {
            let mut sl = StyledLine::new();
            sl.push(
                format!("  {}", line),
                SpanStyle {
                    bg: Some(Color::Ansi256(235)),
                    ..SpanStyle::default()
                },
            );
            sl
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let lines = highlight_code(code, "rust", 80);
        assert_eq!(lines.len(), 3);
        // Each line should have colored spans
        for line in &lines {
            assert!(!line.spans.is_empty());
        }
    }

    #[test]
    fn test_highlight_unknown_language() {
        let code = "some plain text";
        let lines = highlight_code(code, "nonexistent_lang_xyz", 80);
        assert!(!lines.is_empty());
        // Should still render something (plain text fallback)
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn test_highlight_empty_language() {
        let code = "x = 1";
        let lines = highlight_code(code, "", 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_empty_code() {
        let lines = highlight_code("", "rust", 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_python() {
        let code = "def hello():\n    print('world')";
        let lines = highlight_code(code, "py", 80);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_code_has_background() {
        let code = "let x = 1;";
        let lines = highlight_code(code, "rust", 80);
        // All spans should have a background color (code block bg)
        for line in &lines {
            for span in &line.spans {
                assert!(span.style.bg.is_some(), "code span should have background");
            }
        }
    }
}
