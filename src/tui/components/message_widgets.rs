//! Message rendering widgets for the chat transcript.
//!
//! Ports of `ref/components/messages/`:
//! - `UserPromptMessage.tsx` -> [`UserMessageWidget`]
//! - `AssistantTextMessage.tsx` -> [`AssistantTextWidget`]  (with markdown)
//! - `AssistantToolUseMessage.tsx` -> [`ToolUseWidget`]     (bordered box)
//! - `UserToolResultMessage/` -> [`ToolResultWidget`]       (collapsible)
//! - `AssistantThinkingMessage.tsx` -> [`ThinkingWidget`]

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::markdown::{self, SpanStyle, StyledLine, StyledSpan};
use crate::tui::style::{Color, NamedColor};

use super::text_widget::{TextSpan, TextWidget};
use super::Widget;

// ---------------------------------------------------------------------------
// Helpers: convert StyledLine/StyledSpan into DOM nodes
// ---------------------------------------------------------------------------

/// Build a DOM text element from a [`StyledLine`].
fn styled_line_to_node(tree: &mut DomTree, line: &StyledLine) -> NodeId {
    if line.spans.is_empty() {
        // Empty line: just a newline text node
        let node = tree.create_element(ElementType::Text);
        let tn = tree.create_text_node(" ");
        tree.append_child(node, tn);
        return node;
    }

    // Build a TextWidget with spans
    let first = &line.spans[0];
    let rest = &line.spans[1..];

    let tw = TextWidget {
        content: first.text.clone(),
        color: first.style.fg.clone(),
        background_color: first.style.bg.clone(),
        bold: first.style.bold,
        dim: first.style.dim,
        italic: first.style.italic,
        underline: first.style.underline,
        strikethrough: first.style.strikethrough,
        spans: rest
            .iter()
            .map(|s| TextSpan {
                content: s.text.clone(),
                color: s.style.fg.clone(),
                background_color: s.style.bg.clone(),
                bold: s.style.bold,
                dim: s.style.dim,
                italic: s.style.italic,
                underline: s.style.underline,
                strikethrough: s.style.strikethrough,
                inverse: false,
            })
            .collect(),
        ..TextWidget::default()
    };

    tw.build(tree)
}

/// Build a column of DOM nodes from a list of [`StyledLine`]s.
fn styled_lines_to_column(
    tree: &mut DomTree,
    lines: &[StyledLine],
    extra_padding_left: f32,
) -> NodeId {
    let col = tree.create_element(ElementType::Box);
    tree.set_style(
        col,
        LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            padding: Edges {
                left: extra_padding_left,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        },
    );

    for line in lines {
        let line_node = styled_line_to_node(tree, line);
        tree.append_child(col, line_node);
    }

    col
}

// ---------------------------------------------------------------------------
// UserMessageWidget
// ---------------------------------------------------------------------------

/// A user message in the chat transcript.
///
/// Displays with a ">" prompt prefix and dimmed text, matching the
/// `ref/components/messages/UserPromptMessage.tsx` pattern.
#[derive(Debug, Clone)]
pub struct UserMessageWidget {
    /// The user's input text.
    pub text: String,
    /// Whether to add a top margin (not the first message).
    pub add_margin: bool,
}

impl Widget for UserMessageWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Outer row: ">" prefix + text
        let outer = tree.create_element(ElementType::Box);
        let margin_top = if self.add_margin { 1.0 } else { 0.0 };
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            width: Dimension::Percent(100.0),
            margin: Edges {
                top: margin_top,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Prompt indicator: ">" in accent color
        let prompt = TextWidget {
            content: "\u{276F} ".into(),
            color: Some(Color::Named(NamedColor::Magenta)),
            bold: true,
            ..TextWidget::default()
        };
        let prompt_node = prompt.build(tree);
        tree.append_child(outer, prompt_node);

        // User text (dimmed)
        let text = TextWidget::dimmed(&self.text);
        let text_node = text.build(tree);
        tree.append_child(outer, text_node);

        outer
    }
}

// ---------------------------------------------------------------------------
// AssistantTextWidget
// ---------------------------------------------------------------------------

/// An assistant text response in the chat transcript.
///
/// Renders the text as **markdown** with:
/// - Bold, italic, inline code, strikethrough
/// - Headings with color
/// - Code blocks with syntax highlighting (via syntect)
/// - Lists (bullet and numbered)
/// - Blockquotes with left border
/// - Links
///
/// Supports streaming: during streaming, appends a block cursor.
#[derive(Debug, Clone)]
pub struct AssistantTextWidget {
    /// The assistant's response text (may be partial during streaming).
    pub text: String,
    /// Whether to add a top margin.
    pub add_margin: bool,
    /// Whether this message is still streaming (show cursor dot).
    pub is_streaming: bool,
    /// Optional search query to highlight.
    pub search_query: Option<String>,
    /// Available width for markdown rendering.
    pub width: usize,
}

impl Default for AssistantTextWidget {
    fn default() -> Self {
        Self {
            text: String::new(),
            add_margin: false,
            is_streaming: false,
            search_query: None,
            width: 80,
        }
    }
}

impl Widget for AssistantTextWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let outer = tree.create_element(ElementType::Box);
        let margin_top = if self.add_margin { 1.0 } else { 0.0 };
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            margin: Edges {
                top: margin_top,
                ..Edges::default()
            },
            padding: Edges {
                left: 2.0,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Render markdown
        let display_text = if self.is_streaming && !self.text.is_empty() {
            format!("{}\u{2588}", self.text) // block cursor at end
        } else {
            self.text.clone()
        };

        let mut lines = markdown::render_markdown(&display_text, self.width);

        // Apply search highlighting if active
        if let Some(ref query) = self.search_query {
            lines = markdown::highlight_search(&lines, query);
        }

        let content_node = styled_lines_to_column(tree, &lines, 0.0);
        tree.append_child(outer, content_node);

        outer
    }

    fn update(&self, tree: &mut DomTree, node: NodeId) {
        // For streaming, rebuild all children.
        if let Some(elem) = tree.element(node) {
            let children: Vec<NodeId> = elem.children.clone();
            for child in children {
                tree.remove_child(node, child);
            }
        }

        let display_text = if self.is_streaming && !self.text.is_empty() {
            format!("{}\u{2588}", self.text)
        } else {
            self.text.clone()
        };

        let mut lines = markdown::render_markdown(&display_text, self.width);
        if let Some(ref query) = self.search_query {
            lines = markdown::highlight_search(&lines, query);
        }

        let content_node = styled_lines_to_column(tree, &lines, 0.0);
        tree.append_child(node, content_node);
        tree.mark_dirty(node);
    }
}

// ---------------------------------------------------------------------------
// ToolUseWidget
// ---------------------------------------------------------------------------

/// A tool invocation display in the chat transcript.
///
/// Renders a bordered box matching the ref pattern:
/// ```text
/// ┌ ToolName ────────────────────────┐
/// │ key: value                        │
/// └──────────────────────────────────┘
/// ```
///
/// With tool-specific color on the left border and name badge.
#[derive(Debug, Clone)]
pub struct ToolUseWidget {
    /// Tool name (e.g., "Read", "Edit", "Bash").
    pub tool_name: String,
    /// User-facing tool name with parameters (e.g., "Read(src/lib.rs)").
    pub display_name: String,
    /// Color for the tool border and name.
    pub tool_color: Color,
    /// Summary of the tool input parameters.
    pub input_summary: String,
    /// Key-value pairs for structured tool input display.
    pub input_pairs: Vec<(String, String)>,
    /// Whether the tool is still in progress.
    pub in_progress: bool,
    /// Whether to add a top margin.
    pub add_margin: bool,
    /// Available width for box drawing.
    pub width: usize,
}

impl Default for ToolUseWidget {
    fn default() -> Self {
        Self {
            tool_name: String::new(),
            display_name: String::new(),
            tool_color: Color::Named(NamedColor::Cyan),
            input_summary: String::new(),
            input_pairs: Vec::new(),
            in_progress: false,
            add_margin: false,
            width: 80,
        }
    }
}

impl Widget for ToolUseWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let outer = tree.create_element(ElementType::Box);
        let margin_top = if self.add_margin { 1.0 } else { 0.0 };
        tree.set_style(
            outer,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Percent(100.0),
                margin: Edges {
                    top: margin_top,
                    ..Edges::default()
                },
                padding: Edges {
                    left: 2.0,
                    ..Edges::default()
                },
                ..LayoutStyle::default()
            },
        );

        let box_width = self.width.saturating_sub(4).max(20);

        // --- Top border: "| ToolName ---...|"
        let top_border = self.build_top_border(box_width);
        let top_node = styled_line_to_node(tree, &top_border);
        tree.append_child(outer, top_node);

        // --- Input content lines
        let content_lines = self.build_content_lines(box_width);
        for line in &content_lines {
            let line_node = styled_line_to_node(tree, line);
            tree.append_child(outer, line_node);
        }

        // --- Bottom border
        let bottom_border = self.build_bottom_border(box_width);
        let bottom_node = styled_line_to_node(tree, &bottom_border);
        tree.append_child(outer, bottom_node);

        // --- Spinner for in-progress
        if self.in_progress {
            let spinner_tw = TextWidget {
                content: "  \u{22EF} running".into(),
                dim: true,
                italic: true,
                ..TextWidget::default()
            };
            let spinner_node = spinner_tw.build(tree);
            tree.append_child(outer, spinner_node);
        }

        outer
    }
}

impl ToolUseWidget {
    fn build_top_border(&self, box_width: usize) -> StyledLine {
        let mut sl = StyledLine::new();
        // "| "
        sl.push(
            "\u{250C} ",
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                ..SpanStyle::default()
            },
        );
        // Tool name
        sl.push(
            &self.display_name,
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                bold: true,
                ..SpanStyle::default()
            },
        );
        // Fill with dashes
        let name_len = self.display_name.len() + 3; // "| " + name + " "
        let remaining = box_width.saturating_sub(name_len + 1);
        let fill: String = " \u{2500}".to_string()
            + &"\u{2500}".repeat(remaining);
        sl.push(
            fill,
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                dim: true,
                ..SpanStyle::default()
            },
        );
        sl.push(
            "\u{2510}",
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                dim: true,
                ..SpanStyle::default()
            },
        );
        sl
    }

    fn build_bottom_border(&self, box_width: usize) -> StyledLine {
        let mut sl = StyledLine::new();
        sl.push(
            "\u{2514}",
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                dim: true,
                ..SpanStyle::default()
            },
        );
        let fill: String = "\u{2500}".repeat(box_width.saturating_sub(2));
        sl.push(
            fill,
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                dim: true,
                ..SpanStyle::default()
            },
        );
        sl.push(
            "\u{2518}",
            SpanStyle {
                fg: Some(self.tool_color.clone()),
                dim: true,
                ..SpanStyle::default()
            },
        );
        sl
    }

    fn build_content_lines(&self, box_width: usize) -> Vec<StyledLine> {
        let mut lines = Vec::new();
        let inner_width = box_width.saturating_sub(4); // "| " ... " |"

        // Key-value pairs
        if !self.input_pairs.is_empty() {
            for (key, value) in &self.input_pairs {
                let mut sl = StyledLine::new();
                sl.push(
                    "\u{2502} ",
                    SpanStyle {
                        fg: Some(self.tool_color.clone()),
                        dim: true,
                        ..SpanStyle::default()
                    },
                );
                sl.push(
                    format!("{}: ", key),
                    SpanStyle {
                        fg: Some(Color::Named(NamedColor::BrightBlack)),
                        ..SpanStyle::default()
                    },
                );
                // Truncate value if too long
                let max_val_len = inner_width.saturating_sub(key.len() + 2);
                let display_val = if value.len() > max_val_len && max_val_len > 3 {
                    format!("{}...", &value[..max_val_len - 3])
                } else {
                    value.clone()
                };
                sl.push(
                    display_val,
                    SpanStyle::default(),
                );
                lines.push(sl);
            }
        } else if !self.input_summary.is_empty() {
            // Plain text summary, possibly multiline
            for summary_line in self.input_summary.lines() {
                let mut sl = StyledLine::new();
                sl.push(
                    "\u{2502} ",
                    SpanStyle {
                        fg: Some(self.tool_color.clone()),
                        dim: true,
                        ..SpanStyle::default()
                    },
                );
                let display = if summary_line.len() > inner_width && inner_width > 3 {
                    format!("{}...", &summary_line[..inner_width - 3])
                } else {
                    summary_line.to_string()
                };
                sl.push(
                    display,
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
}

// ---------------------------------------------------------------------------
// ToolResultWidget
// ---------------------------------------------------------------------------

/// A tool result display (the output returned by a tool).
///
/// Shows the result content with collapsible behavior:
/// - Collapsed: shows first N lines + "... (X more lines)"
/// - Expanded: shows all content
/// - Error results render in red
/// - Diff results render with structured coloring
#[derive(Debug, Clone)]
pub struct ToolResultWidget {
    /// The tool result text.
    pub content: String,
    /// Whether the result is in error state.
    pub is_error: bool,
    /// Whether the content is collapsed (truncated).
    pub collapsed: bool,
    /// Maximum lines to show when collapsed.
    pub max_collapsed_lines: usize,
    /// Whether the content is a diff (for structured rendering).
    pub is_diff: bool,
    /// Optional search query to highlight.
    pub search_query: Option<String>,
    /// Available width.
    pub width: usize,
}

impl Default for ToolResultWidget {
    fn default() -> Self {
        Self {
            content: String::new(),
            is_error: false,
            collapsed: true,
            max_collapsed_lines: 10,
            is_diff: false,
            search_query: None,
            width: 80,
        }
    }
}

impl Widget for ToolResultWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let outer = tree.create_element(ElementType::Box);
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            padding: Edges {
                left: 4.0,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        if self.content.is_empty() {
            return outer;
        }

        // Determine if content is a diff
        let mut lines = if self.is_diff || self.looks_like_diff() {
            crate::tui::diff_display::render_diff(&self.content)
        } else if self.is_error {
            // Error content in red
            self.content
                .lines()
                .map(|l| StyledLine {
                    spans: vec![StyledSpan {
                        text: l.to_string(),
                        style: SpanStyle {
                            fg: Some(Color::Named(NamedColor::Red)),
                            ..SpanStyle::default()
                        },
                    }],
                })
                .collect()
        } else {
            // Normal result content (dimmed)
            self.content
                .lines()
                .map(|l| StyledLine {
                    spans: vec![StyledSpan {
                        text: l.to_string(),
                        style: SpanStyle {
                            dim: true,
                            ..SpanStyle::default()
                        },
                    }],
                })
                .collect()
        };

        // Apply collapse truncation
        let total_lines = lines.len();
        if self.collapsed && total_lines > self.max_collapsed_lines {
            lines.truncate(self.max_collapsed_lines);
            lines.push(StyledLine {
                spans: vec![StyledSpan {
                    text: format!(
                        "... ({} more lines)",
                        total_lines - self.max_collapsed_lines
                    ),
                    style: SpanStyle {
                        fg: Some(Color::Named(NamedColor::BrightBlack)),
                        dim: true,
                        ..SpanStyle::default()
                    },
                }],
            });
        }

        // Apply search highlighting
        if let Some(ref query) = self.search_query {
            lines = markdown::highlight_search(&lines, query);
        }

        let content_node = styled_lines_to_column(tree, &lines, 0.0);
        tree.append_child(outer, content_node);

        outer
    }
}

impl ToolResultWidget {
    /// Heuristic: does the content look like a unified diff?
    fn looks_like_diff(&self) -> bool {
        let first_lines: String = self.content.lines().take(5).collect::<Vec<_>>().join("\n");
        first_lines.contains("--- ") && first_lines.contains("+++ ")
            || first_lines.starts_with("diff ")
    }
}

// ---------------------------------------------------------------------------
// ThinkingWidget
// ---------------------------------------------------------------------------

/// A "thinking" indicator shown while the model is in extended thinking mode.
#[derive(Debug, Clone)]
pub struct ThinkingWidget {
    /// The thinking text (if verbose mode, show full text).
    pub thinking_text: Option<String>,
    /// Whether to show the full thinking text or just an indicator.
    pub verbose: bool,
    /// Whether to add a top margin.
    pub add_margin: bool,
}

impl Widget for ThinkingWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let outer = tree.create_element(ElementType::Box);
        let margin_top = if self.add_margin { 1.0 } else { 0.0 };
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            margin: Edges {
                top: margin_top,
                ..Edges::default()
            },
            padding: Edges {
                left: 2.0,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        if self.verbose {
            // Show full thinking text
            if let Some(ref text) = self.thinking_text {
                let tw = TextWidget {
                    content: text.clone(),
                    dim: true,
                    italic: true,
                    ..TextWidget::default()
                };
                let text_node = tw.build(tree);
                tree.append_child(outer, text_node);
            }
        } else {
            // Compact indicator: ":: Thinking"
            let indicator = TextWidget {
                content: "\u{2234} Thinking".into(),
                dim: true,
                italic: true,
                ..TextWidget::default()
            };
            let indicator_node = indicator.build(tree);
            tree.append_child(outer, indicator_node);
        }

        outer
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message_build() {
        let mut tree = DomTree::new();
        let w = UserMessageWidget {
            text: "hello".into(),
            add_margin: true,
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // prompt + text = 2 children
        assert_eq!(elem.children.len(), 2);
    }

    #[test]
    fn test_user_message_prompt_color() {
        let mut tree = DomTree::new();
        let w = UserMessageWidget {
            text: "test".into(),
            add_margin: false,
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // First child is the prompt
        let prompt_node = elem.children[0];
        let prompt_elem = tree.element(prompt_node).unwrap();
        let styles = prompt_elem.text_styles.as_ref().unwrap();
        assert_eq!(styles.color, Some(Color::Named(NamedColor::Magenta)));
    }

    #[test]
    fn test_assistant_text_build() {
        let mut tree = DomTree::new();
        let w = AssistantTextWidget {
            text: "response".into(),
            add_margin: false,
            is_streaming: false,
            ..AssistantTextWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 1);
    }

    #[test]
    fn test_assistant_text_markdown() {
        let mut tree = DomTree::new();
        let w = AssistantTextWidget {
            text: "**bold** and *italic*".into(),
            add_margin: false,
            is_streaming: false,
            ..AssistantTextWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // Should have a column child with styled lines
        assert!(!elem.children.is_empty());
    }

    #[test]
    fn test_assistant_text_code_block() {
        let mut tree = DomTree::new();
        let w = AssistantTextWidget {
            text: "```rust\nfn main() {}\n```".into(),
            add_margin: false,
            is_streaming: false,
            ..AssistantTextWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert!(!elem.children.is_empty());
    }

    #[test]
    fn test_assistant_text_streaming() {
        let mut tree = DomTree::new();
        let w = AssistantTextWidget {
            text: "partial".into(),
            add_margin: false,
            is_streaming: true,
            ..AssistantTextWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 1);
    }

    #[test]
    fn test_assistant_text_update() {
        let mut tree = DomTree::new();
        let w1 = AssistantTextWidget {
            text: "partial".into(),
            add_margin: false,
            is_streaming: true,
            ..AssistantTextWidget::default()
        };
        let node = w1.build(&mut tree);

        let w2 = AssistantTextWidget {
            text: "partial response complete".into(),
            add_margin: false,
            is_streaming: false,
            ..AssistantTextWidget::default()
        };
        w2.update(&mut tree, node);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 1);
    }

    #[test]
    fn test_assistant_text_search_highlight() {
        let mut tree = DomTree::new();
        let w = AssistantTextWidget {
            text: "the quick brown fox".into(),
            search_query: Some("quick".into()),
            ..AssistantTextWidget::default()
        };
        let node = w.build(&mut tree);
        assert!(tree.element(node).is_some());
    }

    #[test]
    fn test_tool_use_build() {
        let mut tree = DomTree::new();
        let w = ToolUseWidget {
            tool_name: "Read".into(),
            display_name: "Read(src/lib.rs)".into(),
            tool_color: Color::Named(NamedColor::Cyan),
            input_summary: String::new(),
            in_progress: false,
            add_margin: true,
            ..ToolUseWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // Should have: top border + bottom border = at least 2 children
        assert!(elem.children.len() >= 2);
    }

    #[test]
    fn test_tool_use_with_pairs() {
        let mut tree = DomTree::new();
        let w = ToolUseWidget {
            tool_name: "Edit".into(),
            display_name: "Edit".into(),
            tool_color: Color::Named(NamedColor::Yellow),
            input_pairs: vec![
                ("file".into(), "src/main.rs".into()),
                ("old".into(), "foo".into()),
                ("new".into(), "bar".into()),
            ],
            ..ToolUseWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // top border + 3 content lines + bottom border = 5
        assert_eq!(elem.children.len(), 5);
    }

    #[test]
    fn test_tool_use_in_progress() {
        let mut tree = DomTree::new();
        let w = ToolUseWidget {
            tool_name: "Bash".into(),
            display_name: "Bash".into(),
            tool_color: Color::Named(NamedColor::Red),
            in_progress: true,
            ..ToolUseWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // top border + bottom border + spinner = 3
        assert!(elem.children.len() >= 3);
    }

    #[test]
    fn test_tool_result_collapsed() {
        let mut tree = DomTree::new();
        let long_content = (0..20)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let w = ToolResultWidget {
            content: long_content,
            collapsed: true,
            max_collapsed_lines: 5,
            ..ToolResultWidget::default()
        };
        let node = w.build(&mut tree);
        assert!(tree.element(node).is_some());
    }

    #[test]
    fn test_tool_result_error() {
        let mut tree = DomTree::new();
        let w = ToolResultWidget {
            content: "error: something went wrong".into(),
            is_error: true,
            ..ToolResultWidget::default()
        };
        let node = w.build(&mut tree);
        assert!(tree.element(node).is_some());
    }

    #[test]
    fn test_tool_result_diff_detection() {
        let w = ToolResultWidget {
            content: "--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n+new".into(),
            ..ToolResultWidget::default()
        };
        assert!(w.looks_like_diff());
    }

    #[test]
    fn test_tool_result_not_diff() {
        let w = ToolResultWidget {
            content: "hello world".into(),
            ..ToolResultWidget::default()
        };
        assert!(!w.looks_like_diff());
    }

    #[test]
    fn test_tool_result_empty() {
        let mut tree = DomTree::new();
        let w = ToolResultWidget {
            content: String::new(),
            ..ToolResultWidget::default()
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert!(elem.children.is_empty());
    }

    #[test]
    fn test_thinking_compact() {
        let mut tree = DomTree::new();
        let w = ThinkingWidget {
            thinking_text: Some("deep thoughts".into()),
            verbose: false,
            add_margin: false,
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 1);
    }

    #[test]
    fn test_thinking_verbose() {
        let mut tree = DomTree::new();
        let w = ThinkingWidget {
            thinking_text: Some("deep thoughts about code".into()),
            verbose: true,
            add_margin: true,
        };
        let node = w.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 1);
    }
}
