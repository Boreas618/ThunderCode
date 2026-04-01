//! Markdown-to-styled-text converter.
//!
//! Parses markdown using `pulldown-cmark` and produces [`StyledLine`]s with
//! bold, italic, inline code, headings, lists, blockquotes, and fenced code
//! blocks (delegating to [`crate::tui::syntax_highlight`] for coloring).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, CodeBlockKind, HeadingLevel};

use crate::tui::style::{Color, NamedColor};
use crate::tui::syntax_highlight;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single styled span of text within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub style: SpanStyle,
}

/// Visual style for a span.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpanStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// One logical line of styled output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn plain(text: &str) -> Self {
        Self {
            spans: vec![StyledSpan {
                text: text.to_string(),
                style: SpanStyle::default(),
            }],
        }
    }

    /// Total visible character count (no ANSI).
    pub fn text_len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty() || self.spans.iter().all(|s| s.text.is_empty())
    }

    /// Push a styled span onto this line.
    pub fn push(&mut self, text: impl Into<String>, style: SpanStyle) {
        let text = text.into();
        if !text.is_empty() {
            self.spans.push(StyledSpan { text, style });
        }
    }

    /// Push plain (unstyled) text.
    pub fn push_plain(&mut self, text: impl Into<String>) {
        self.push(text, SpanStyle::default());
    }

    /// Convert this line to an ANSI-escaped string.
    pub fn to_ansi(&self) -> String {
        let mut out = String::new();
        for span in &self.spans {
            let s = &span.style;
            let mut codes: Vec<String> = Vec::new();
            if s.bold { codes.push("1".into()); }
            if s.dim { codes.push("2".into()); }
            if s.italic { codes.push("3".into()); }
            if s.underline { codes.push("4".into()); }
            if s.strikethrough { codes.push("9".into()); }
            if let Some(ref fg) = s.fg {
                match fg {
                    Color::Rgb(r, g, b) => codes.push(format!("38;2;{r};{g};{b}")),
                    Color::Ansi256(n) => codes.push(format!("38;5;{n}")),
                    Color::Named(n) => codes.push(n.fg_code().to_string()),
                }
            }
            if let Some(ref bg) = s.bg {
                match bg {
                    Color::Rgb(r, g, b) => codes.push(format!("48;2;{r};{g};{b}")),
                    Color::Ansi256(n) => codes.push(format!("48;5;{n}")),
                    Color::Named(n) => codes.push(n.bg_code().to_string()),
                }
            }
            if codes.is_empty() {
                out.push_str(&span.text);
            } else {
                out.push_str(&format!("\x1b[{}m{}\x1b[0m", codes.join(";"), span.text));
            }
        }
        out
    }
}

impl Default for StyledLine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Markdown rendering
// ---------------------------------------------------------------------------

/// Render a markdown string into styled lines.
///
/// `width` is the available terminal width (used for code block wrapping).
pub fn render_markdown(text: &str, width: usize) -> Vec<StyledLine> {
    let mut renderer = MarkdownRenderer::new(width);
    renderer.render(text);
    renderer.lines
}

struct MarkdownRenderer {
    lines: Vec<StyledLine>,
    width: usize,
    // Formatting stack
    bold: bool,
    italic: bool,
    strikethrough: bool,
    // Block state
    in_code_block: bool,
    code_block_lang: String,
    code_block_buf: String,
    in_heading: Option<HeadingLevel>,
    in_blockquote: bool,
    blockquote_depth: usize,
    // List state
    list_stack: Vec<ListContext>,
    // Inline code
    in_inline_code: bool,
    // Link
    in_link: bool,
    link_url: String,
    // Paragraph tracking
    needs_blank_line: bool,
    // Current line accumulator
    current_line: StyledLine,
}

#[derive(Debug, Clone)]
struct ListContext {
    ordered: bool,
    next_number: u64,
    indent: usize,
}

impl MarkdownRenderer {
    fn new(width: usize) -> Self {
        Self {
            lines: Vec::new(),
            width,
            bold: false,
            italic: false,
            strikethrough: false,
            in_code_block: false,
            code_block_lang: String::new(),
            code_block_buf: String::new(),
            in_heading: None,
            in_blockquote: false,
            blockquote_depth: 0,
            list_stack: Vec::new(),
            in_inline_code: false,
            in_link: false,
            link_url: String::new(),
            needs_blank_line: false,
            current_line: StyledLine::new(),
        }
    }

    fn render(&mut self, text: &str) {
        let opts = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_HEADING_ATTRIBUTES;
        let parser = Parser::new_ext(text, opts);
        let events: Vec<Event> = parser.collect();

        for event in events {
            match event {
                Event::Start(tag) => self.start_tag(tag),
                Event::End(tag) => self.end_tag(tag),
                Event::Text(text) => self.handle_text(&text),
                Event::Code(code) => self.handle_inline_code(&code),
                Event::SoftBreak => self.handle_soft_break(),
                Event::HardBreak => self.flush_line(),
                Event::Rule => self.handle_rule(),
                _ => {}
            }
        }

        // Flush any remaining content
        if !self.current_line.is_empty() {
            self.flush_line();
        }

        // Trim trailing empty lines
        while self.lines.last().map_or(false, |l| l.is_empty()) {
            self.lines.pop();
        }
    }

    fn current_style(&self) -> SpanStyle {
        if self.in_heading.is_some() {
            SpanStyle {
                fg: Some(Color::Named(NamedColor::Cyan)),
                bold: true,
                ..SpanStyle::default()
            }
        } else if self.in_inline_code {
            SpanStyle {
                bg: Some(Color::Ansi256(236)), // dark gray background
                ..SpanStyle::default()
            }
        } else if self.in_link {
            SpanStyle {
                fg: Some(Color::Named(NamedColor::Blue)),
                underline: true,
                ..SpanStyle::default()
            }
        } else {
            SpanStyle {
                bold: self.bold,
                italic: self.italic,
                strikethrough: self.strikethrough,
                ..SpanStyle::default()
            }
        }
    }

    fn flush_line(&mut self) {
        let mut line = std::mem::take(&mut self.current_line);
        // Prefix blockquote lines
        if self.in_blockquote && self.blockquote_depth > 0 {
            let prefix_spans = self.blockquote_prefix();
            let mut new_line = StyledLine::new();
            for s in prefix_spans {
                new_line.spans.push(s);
            }
            for s in line.spans.drain(..) {
                new_line.spans.push(s);
            }
            line = new_line;
        }
        self.lines.push(line);
    }

    fn blockquote_prefix(&self) -> Vec<StyledSpan> {
        let mut spans = Vec::new();
        for _ in 0..self.blockquote_depth {
            spans.push(StyledSpan {
                text: "\u{2502} ".to_string(), // "| "
                style: SpanStyle {
                    fg: Some(Color::Named(NamedColor::BrightBlack)),
                    dim: true,
                    ..SpanStyle::default()
                },
            });
        }
        spans
    }

    fn emit_blank_line_if_needed(&mut self) {
        if self.needs_blank_line && !self.lines.is_empty() {
            // Only add blank line if last line isn't already blank
            if !self.lines.last().map_or(true, |l| l.is_empty()) {
                self.lines.push(StyledLine::new());
            }
            self.needs_blank_line = false;
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.emit_blank_line_if_needed();
            }
            Tag::Heading { level, .. } => {
                self.emit_blank_line_if_needed();
                self.in_heading = Some(level);
                // Add heading prefix
                let prefix = match level {
                    HeadingLevel::H1 => "# ",
                    HeadingLevel::H2 => "## ",
                    HeadingLevel::H3 => "### ",
                    HeadingLevel::H4 => "#### ",
                    HeadingLevel::H5 => "##### ",
                    HeadingLevel::H6 => "###### ",
                };
                self.current_line.push(
                    prefix,
                    SpanStyle {
                        fg: Some(Color::Named(NamedColor::Cyan)),
                        bold: true,
                        ..SpanStyle::default()
                    },
                );
            }
            Tag::BlockQuote(_) => {
                self.emit_blank_line_if_needed();
                self.in_blockquote = true;
                self.blockquote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.emit_blank_line_if_needed();
                self.in_code_block = true;
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code_block_buf.clear();
            }
            Tag::List(first) => {
                self.emit_blank_line_if_needed();
                let indent = self.list_stack.len() * 2;
                self.list_stack.push(ListContext {
                    ordered: first.is_some(),
                    next_number: first.unwrap_or(1),
                    indent,
                });
            }
            Tag::Item => {
                // Flush any existing content on current line
                if !self.current_line.is_empty() {
                    self.flush_line();
                }
                // Add bullet/number prefix
                if let Some(ctx) = self.list_stack.last_mut() {
                    let indent = " ".repeat(ctx.indent);
                    let bullet = if ctx.ordered {
                        let n = ctx.next_number;
                        ctx.next_number += 1;
                        format!("{}{}. ", indent, n)
                    } else {
                        format!("{}\u{2022} ", indent) // bullet character
                    };
                    self.current_line.push(
                        bullet,
                        SpanStyle {
                            fg: Some(Color::Named(NamedColor::BrightBlack)),
                            ..SpanStyle::default()
                        },
                    );
                }
            }
            Tag::Emphasis => {
                self.italic = true;
            }
            Tag::Strong => {
                self.bold = true;
            }
            Tag::Strikethrough => {
                self.strikethrough = true;
            }
            Tag::Link { dest_url, .. } => {
                self.in_link = true;
                self.link_url = dest_url.to_string();
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line();
                self.needs_blank_line = true;
            }
            TagEnd::Heading(_) => {
                self.in_heading = None;
                self.flush_line();
                self.needs_blank_line = true;
            }
            TagEnd::BlockQuote(_) => {
                if self.blockquote_depth > 0 {
                    self.blockquote_depth -= 1;
                }
                if self.blockquote_depth == 0 {
                    self.in_blockquote = false;
                }
                self.flush_line();
                self.needs_blank_line = true;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let lang = std::mem::take(&mut self.code_block_lang);
                let code = std::mem::take(&mut self.code_block_buf);

                // Language label
                if !lang.is_empty() {
                    self.lines.push(StyledLine {
                        spans: vec![StyledSpan {
                            text: format!(" {} ", lang),
                            style: SpanStyle {
                                fg: Some(Color::Named(NamedColor::BrightBlack)),
                                bg: Some(Color::Ansi256(236)),
                                ..SpanStyle::default()
                            },
                        }],
                    });
                }

                // Syntax-highlighted code
                let highlighted = syntax_highlight::highlight_code(
                    &code,
                    &lang,
                    self.width,
                );
                for hl_line in highlighted {
                    self.lines.push(hl_line);
                }

                self.needs_blank_line = true;
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if !self.current_line.is_empty() {
                    self.flush_line();
                }
                self.needs_blank_line = true;
            }
            TagEnd::Item => {
                if !self.current_line.is_empty() {
                    self.flush_line();
                }
            }
            TagEnd::Emphasis => {
                self.italic = false;
            }
            TagEnd::Strong => {
                self.bold = false;
            }
            TagEnd::Strikethrough => {
                self.strikethrough = false;
            }
            TagEnd::Link => {
                // Append the URL after link text
                if !self.link_url.is_empty() {
                    self.current_line.push(
                        format!(" ({})", self.link_url),
                        SpanStyle {
                            fg: Some(Color::Named(NamedColor::BrightBlack)),
                            dim: true,
                            ..SpanStyle::default()
                        },
                    );
                }
                self.in_link = false;
                self.link_url.clear();
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_buf.push_str(text);
            return;
        }

        // Split by newlines to handle multiline text within a single event
        let mut first = true;
        for line in text.split('\n') {
            if !first {
                self.flush_line();
            }
            first = false;
            if !line.is_empty() {
                self.current_line.push(line.to_string(), self.current_style());
            }
        }
    }

    fn handle_inline_code(&mut self, code: &str) {
        self.current_line.push(
            format!(" {} ", code),
            SpanStyle {
                bg: Some(Color::Ansi256(236)),
                ..SpanStyle::default()
            },
        );
    }

    fn handle_soft_break(&mut self) {
        // Treat soft breaks as spaces in inline context
        self.current_line.push(" ", self.current_style());
    }

    fn handle_rule(&mut self) {
        self.flush_line();
        let rule_char = "\u{2500}"; // ─
        let line_str: String = std::iter::repeat(rule_char)
            .take(self.width.min(80))
            .collect();
        self.lines.push(StyledLine {
            spans: vec![StyledSpan {
                text: line_str,
                style: SpanStyle {
                    fg: Some(Color::Named(NamedColor::BrightBlack)),
                    dim: true,
                    ..SpanStyle::default()
                },
            }],
        });
        self.needs_blank_line = true;
    }
}

// ---------------------------------------------------------------------------
// Search highlighting
// ---------------------------------------------------------------------------

/// Apply search-match highlighting (yellow background) to styled lines.
///
/// Returns a new set of lines with matching substrings highlighted.
pub fn highlight_search(lines: &[StyledLine], query: &str) -> Vec<StyledLine> {
    if query.is_empty() {
        return lines.to_vec();
    }

    let query_lower = query.to_lowercase();
    let highlight_style = SpanStyle {
        bg: Some(Color::Named(NamedColor::Yellow)),
        fg: Some(Color::Named(NamedColor::Black)),
        bold: true,
        ..SpanStyle::default()
    };

    lines
        .iter()
        .map(|line| {
            let mut new_spans = Vec::new();
            for span in &line.spans {
                highlight_span(span, &query_lower, &highlight_style, &mut new_spans);
            }
            StyledLine { spans: new_spans }
        })
        .collect()
}

/// Split a single span, inserting highlight style wherever `query` matches.
fn highlight_span(
    span: &StyledSpan,
    query_lower: &str,
    highlight_style: &SpanStyle,
    out: &mut Vec<StyledSpan>,
) {
    let text_lower = span.text.to_lowercase();
    let mut start = 0;

    loop {
        match text_lower[start..].find(query_lower) {
            Some(pos) => {
                let abs_pos = start + pos;
                // Text before match
                if abs_pos > start {
                    out.push(StyledSpan {
                        text: span.text[start..abs_pos].to_string(),
                        style: span.style.clone(),
                    });
                }
                // The matched text with highlight
                out.push(StyledSpan {
                    text: span.text[abs_pos..abs_pos + query_lower.len()].to_string(),
                    style: highlight_style.clone(),
                });
                start = abs_pos + query_lower.len();
            }
            None => {
                // Remainder
                if start < span.text.len() {
                    out.push(StyledSpan {
                        text: span.text[start..].to_string(),
                        style: span.style.clone(),
                    });
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let lines = render_markdown("hello world", 80);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].text, "hello world");
    }

    #[test]
    fn test_bold() {
        let lines = render_markdown("**bold**", 80);
        assert!(!lines.is_empty());
        // Should have a bold span
        let has_bold = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.bold && s.text.contains("bold"));
        assert!(has_bold);
    }

    #[test]
    fn test_italic() {
        let lines = render_markdown("*italic*", 80);
        let has_italic = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.italic && s.text.contains("italic"));
        assert!(has_italic);
    }

    #[test]
    fn test_inline_code() {
        let lines = render_markdown("use `foo` here", 80);
        let has_code_bg = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.bg.is_some() && s.text.contains("foo"));
        assert!(has_code_bg);
    }

    #[test]
    fn test_heading() {
        let lines = render_markdown("# Title\n\nBody text", 80);
        assert!(lines.len() >= 2);
        // First line should be the heading with bold+cyan
        let heading_span = &lines[0].spans[0];
        assert!(heading_span.style.bold);
        assert_eq!(heading_span.style.fg, Some(Color::Named(NamedColor::Cyan)));
    }

    #[test]
    fn test_bullet_list() {
        let lines = render_markdown("- item one\n- item two", 80);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_numbered_list() {
        let lines = render_markdown("1. first\n2. second", 80);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_blockquote() {
        let lines = render_markdown("> quoted text", 80);
        assert!(!lines.is_empty());
        // Should have the blockquote border character
        let has_border = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.text.contains('\u{2502}'));
        assert!(has_border);
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md, 80);
        // Should have language label + code line(s)
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_rule() {
        let lines = render_markdown("---", 80);
        assert!(!lines.is_empty());
        let has_rule = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.text.contains('\u{2500}'));
        assert!(has_rule);
    }

    #[test]
    fn test_search_highlight() {
        let lines = vec![StyledLine::plain("hello world foo")];
        let highlighted = highlight_search(&lines, "world");
        assert_eq!(highlighted.len(), 1);
        // Should split into 3 spans: "hello ", "world", " foo"
        assert!(highlighted[0].spans.len() >= 2);
        let match_span = highlighted[0]
            .spans
            .iter()
            .find(|s| s.text == "world")
            .unwrap();
        assert_eq!(
            match_span.style.bg,
            Some(Color::Named(NamedColor::Yellow))
        );
    }

    #[test]
    fn test_search_highlight_case_insensitive() {
        let lines = vec![StyledLine::plain("Hello World")];
        let highlighted = highlight_search(&lines, "hello");
        let match_span = highlighted[0]
            .spans
            .iter()
            .find(|s| s.text == "Hello")
            .unwrap();
        assert_eq!(
            match_span.style.bg,
            Some(Color::Named(NamedColor::Yellow))
        );
    }

    #[test]
    fn test_empty_search() {
        let lines = vec![StyledLine::plain("text")];
        let highlighted = highlight_search(&lines, "");
        assert_eq!(highlighted, lines);
    }

    #[test]
    fn test_link() {
        let lines = render_markdown("[click](https://example.com)", 80);
        assert!(!lines.is_empty());
        let has_link_text = lines
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.text.contains("click"));
        assert!(has_link_text);
    }
}
