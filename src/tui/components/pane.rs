//! Bordered pane with a colored top border -- like a card/panel.
//!
//! Port of `ref/components/Pane.tsx`.

use crate::tui::dom::{DomNodeAttribute, DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::{Color, TextStyles};

use super::{BorderStyle, Widget};

/// A bordered card panel with a colored top accent border.
///
/// Used to visually group content (messages, tool output, etc.) with
/// a distinctive top-border color that indicates the content type.
#[derive(Debug, Clone)]
pub struct Pane {
    /// Color of the top accent border (heavy horizontal line).
    pub accent_color: Color,
    /// Optional title displayed in the top border.
    pub title: Option<String>,
    /// Border style for the remaining three sides.
    pub border_style: BorderStyle,
    /// Width of the pane. `None` means 100%.
    pub width: Dimension,
    /// Pre-built child node IDs.
    pub children: Vec<NodeId>,
}

impl Default for Pane {
    fn default() -> Self {
        Self {
            accent_color: Color::Rgb(180, 130, 240), // primary purple
            title: None,
            border_style: BorderStyle::Round,
            width: Dimension::Percent(100.0),
            children: Vec::new(),
        }
    }
}

impl Widget for Pane {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Outer container
        let outer = tree.create_element(ElementType::Box);
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: self.width,
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Top accent line: heavy horizontal characters in the accent color.
        let accent_node = tree.create_element(ElementType::Text);
        let accent_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            width: Dimension::Percent(100.0),
            ..LayoutStyle::default()
        };
        tree.set_style(accent_node, accent_style);
        tree.set_text_styles(
            accent_node,
            TextStyles {
                color: Some(self.accent_color.clone()),
                ..TextStyles::default()
            },
        );

        // Build accent text: ━━━ title ━━━  or just ━━━━━━
        let accent_text = if let Some(ref title) = self.title {
            format!("\u{2501}\u{2501} {} \u{2501}\u{2501}", title)
        } else {
            "\u{2501}".repeat(4)
        };
        let accent_text_node = tree.create_text_node(&accent_text);
        tree.append_child(accent_node, accent_text_node);
        tree.append_child(outer, accent_node);

        // Content area with side borders.
        let content = tree.create_element(ElementType::Box);
        let border_width = if self.border_style != BorderStyle::None {
            1.0
        } else {
            0.0
        };
        let content_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            padding: Edges {
                top: 0.0,
                bottom: 0.0,
                left: 1.0,
                right: 1.0,
            },
            border: Edges {
                top: 0.0,
                bottom: border_width,
                left: border_width,
                right: border_width,
            },
            ..LayoutStyle::default()
        };
        tree.set_style(content, content_style);

        if self.border_style != BorderStyle::None {
            let style_name = match self.border_style {
                BorderStyle::Single => "single",
                BorderStyle::Double => "double",
                BorderStyle::Round => "round",
                BorderStyle::Heavy => "heavy",
                BorderStyle::Ascii => "ascii",
                BorderStyle::None => unreachable!(),
            };
            tree.set_attribute(
                content,
                "borderStyle",
                DomNodeAttribute::String(style_name.into()),
            );
        }

        // Append children into the content area.
        for &child_id in &self.children {
            tree.append_child(content, child_id);
        }
        tree.append_child(outer, content);

        outer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::NamedColor;

    #[test]
    fn test_pane_build() {
        let mut tree = DomTree::new();
        let child = tree.create_text_node("body text");
        let pane = Pane {
            accent_color: Color::Named(NamedColor::Cyan),
            title: Some("Tool Result".into()),
            children: vec![child],
            ..Pane::default()
        };
        let node = pane.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // outer has accent line + content area
        assert_eq!(elem.children.len(), 2);
    }

    #[test]
    fn test_pane_no_title() {
        let mut tree = DomTree::new();
        let pane = Pane::default();
        let node = pane.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.children.len(), 2);
    }
}
