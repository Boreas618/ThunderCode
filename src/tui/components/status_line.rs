//! Status line -- bottom bar showing model name, cost, token usage.
//!
//! Port of `ref/components/StatusLine.tsx`.  Renders a single-row bar
//! with model name on the left and token/cost info on the right.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::text::string_width;

use super::text_widget::TextWidget;
use super::Widget;

/// Data displayed in the status line.
#[derive(Clone)]
pub struct StatusLineData {
    pub model: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub session_info: Option<String>,
    pub permission_mode: Option<String>,
}

impl Default for StatusLineData {
    fn default() -> Self {
        Self {
            model: String::new(),
            cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            session_info: None,
            permission_mode: None,
        }
    }
}

/// Status line widget for the bottom of the screen.
pub struct StatusLine {
    pub data: StatusLineData,
}

impl StatusLine {
    /// Format token count with K/M suffix.
    #[allow(dead_code)]
    fn format_tokens(count: u64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
    }

    /// Render as an ANSI string for direct output (non-DOM path).
    ///
    /// Ref StatusLine: model · permission_mode · tokens · $cost · cwd
    /// All dimmed. Separator is middle dot (·).
    pub fn render_ansi(data: &StatusLineData, width: usize) -> String {
        let dim = "\x1b[2m";
        let rst = "\x1b[0m";
        let sep = " \u{00B7} "; // · middle dot separator

        let mut parts: Vec<String> = Vec::new();

        // Model name
        if !data.model.is_empty() {
            parts.push(data.model.clone());
        }

        // Permission mode
        if let Some(ref mode) = data.permission_mode {
            parts.push(mode.clone());
        }

        // Token usage
        if data.input_tokens > 0 || data.output_tokens > 0 {
            let total = data.input_tokens + data.output_tokens;
            parts.push(format!("{}tok", Self::format_tokens(total)));
        }

        // Cost
        if data.cost_usd > 0.001 {
            parts.push(format!("${:.2}", data.cost_usd));
        }

        // CWD (shortened)
        if let Some(ref info) = data.session_info {
            parts.push(info.clone());
        }

        let content = parts.join(sep);
        let content_width = string_width(&content);

        if content_width >= width {
            format!("{dim}{}{rst}", &content[..width.saturating_sub(1)])
        } else {
            // Right-align, matching ref footer layout
            let pad = width.saturating_sub(content_width);
            format!("{:pad$}{dim}{content}{rst}", "", pad = pad)
        }
    }
}

impl Widget for StatusLine {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let data = &self.data;

        // Ref: PromptInputFooter renders two sides:
        //   Left: "? for shortcuts" (dimmed) or status line content
        //   Right: model info, cost (dimmed)
        //
        // Layout: row with left and spacer and right.
        let outer = tree.create_element(ElementType::Box);
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            width: Dimension::Percent(100.0),
            height: Dimension::Points(1.0),
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Left side: "? for shortcuts"
        let left_tw = TextWidget {
            content: "? for shortcuts".into(),
            dim: true,
            ..TextWidget::default()
        };
        let left_node = left_tw.build(tree);
        tree.append_child(outer, left_node);

        // Spacer to push right side
        let spacer = tree.create_element(ElementType::Box);
        tree.set_style(
            spacer,
            LayoutStyle {
                flex_grow: 1.0,
                ..LayoutStyle::default()
            },
        );
        tree.append_child(outer, spacer);

        // Right side: model name · $cost
        let mut right_parts: Vec<String> = Vec::new();
        if !data.model.is_empty() {
            right_parts.push(data.model.clone());
        }
        if data.cost_usd > 0.0 {
            right_parts.push(format!("${:.4}", data.cost_usd));
        }
        if !right_parts.is_empty() {
            let right_tw = TextWidget {
                content: right_parts.join(" \u{00B7} "),
                dim: true,
                ..TextWidget::default()
            };
            let right_node = right_tw.build(tree);
            tree.append_child(outer, right_node);
        }

        outer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens() {
        assert_eq!(StatusLine::format_tokens(500), "500");
        assert_eq!(StatusLine::format_tokens(1500), "1.5K");
        assert_eq!(StatusLine::format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn test_status_line_build() {
        let mut tree = DomTree::new();
        let sl = StatusLine {
            data: StatusLineData {
                model: "gpt-4o".into(),
                cost_usd: 0.0125,
                input_tokens: 5000,
                output_tokens: 1200,
                session_info: None,
                permission_mode: Some("auto".into()),
            },
        };
        let node = sl.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // model + cost + tokens + spacer + mode = 5
        assert!(elem.children.len() >= 3);
    }

    #[test]
    fn test_render_ansi() {
        let data = StatusLineData {
            model: "test-model".into(),
            cost_usd: 0.05,
            input_tokens: 10_000,
            output_tokens: 2_000,
            session_info: Some("session-1".into()),
            permission_mode: None,
        };
        let output = StatusLine::render_ansi(&data, 80);
        assert!(!output.is_empty());
    }
}
