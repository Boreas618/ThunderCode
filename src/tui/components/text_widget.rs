//! Text (styled text) widget -- the Rust equivalent of Ink's `<Text>`.
//!
//! Produces a DOM `Text` element with styling (color, bold, dim, italic,
//! underline, strikethrough, inverse) and text wrap modes.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::{Color, TextStyles};
use crate::tui::text::TextWrap;

use super::Widget;

/// A styled text span.
///
/// Port of `ref/ink/components/Text.tsx`.
#[derive(Debug, Clone, Default)]
pub struct TextWidget {
    /// The text content to display.
    pub content: String,

    // -- Text styles --
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,

    /// Text wrap mode. Default is `Wrap`.
    pub wrap: TextWrap,

    /// Nested styled spans within this text.
    pub spans: Vec<TextSpan>,
}

/// A styled sub-span within a [`TextWidget`].
#[derive(Debug, Clone, Default)]
pub struct TextSpan {
    pub content: String,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
}

impl TextWidget {
    /// Create a plain text widget with no styling.
    pub fn plain(s: impl Into<String>) -> Self {
        Self {
            content: s.into(),
            ..Default::default()
        }
    }

    /// Create a text widget with foreground color.
    pub fn colored(s: impl Into<String>, color: Color) -> Self {
        Self {
            content: s.into(),
            color: Some(color),
            ..Default::default()
        }
    }

    /// Create a dimmed text widget.
    pub fn dimmed(s: impl Into<String>) -> Self {
        Self {
            content: s.into(),
            dim: true,
            ..Default::default()
        }
    }

    /// Create a bold text widget.
    pub fn bold(s: impl Into<String>) -> Self {
        Self {
            content: s.into(),
            bold: true,
            ..Default::default()
        }
    }

    fn to_text_styles(&self) -> TextStyles {
        TextStyles {
            color: self.color.clone(),
            background_color: self.background_color.clone(),
            bold: if self.bold { Some(true) } else { None },
            dim: if self.dim { Some(true) } else { None },
            italic: if self.italic { Some(true) } else { None },
            underline: if self.underline { Some(true) } else { None },
            strikethrough: if self.strikethrough { Some(true) } else { None },
            inverse: if self.inverse { Some(true) } else { None },
        }
    }
}

impl TextSpan {
    fn to_text_styles(&self) -> TextStyles {
        TextStyles {
            color: self.color.clone(),
            background_color: self.background_color.clone(),
            bold: if self.bold { Some(true) } else { None },
            dim: if self.dim { Some(true) } else { None },
            italic: if self.italic { Some(true) } else { None },
            underline: if self.underline { Some(true) } else { None },
            strikethrough: if self.strikethrough { Some(true) } else { None },
            inverse: if self.inverse { Some(true) } else { None },
        }
    }
}

impl Widget for TextWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // The outer element is a `Text` type with row direction (like Ink).
        let node = tree.create_element(ElementType::Text);
        let style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            flex_shrink: 1.0,
            text_wrap: self.wrap,
            ..LayoutStyle::default()
        };
        tree.set_style(node, style);
        tree.set_text_styles(node, self.to_text_styles());

        // Main content as a text node child.
        if !self.content.is_empty() {
            let text_node = tree.create_text_node(&self.content);
            tree.append_child(node, text_node);
        }

        // Nested spans: each becomes a VirtualText element wrapping a text node.
        for span in &self.spans {
            let span_elem = tree.create_element(ElementType::VirtualText);
            tree.set_text_styles(span_elem, span.to_text_styles());
            let span_text = tree.create_text_node(&span.content);
            tree.append_child(span_elem, span_text);
            tree.append_child(node, span_elem);
        }

        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dom::DomNode;
    use crate::tui::style::NamedColor;

    #[test]
    fn test_text_widget_plain() {
        let mut tree = DomTree::new();
        let tw = TextWidget::plain("hello world");
        let node = tw.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Text);
        // One text-node child
        assert_eq!(elem.children.len(), 1);
        match tree.get(elem.children[0]) {
            Some(DomNode::Text(t)) => assert_eq!(t.value, "hello world"),
            _ => panic!("expected text node"),
        }
    }

    #[test]
    fn test_text_widget_colored() {
        let mut tree = DomTree::new();
        let tw = TextWidget::colored("red text", Color::Named(NamedColor::Red));
        let node = tw.build(&mut tree);
        let elem = tree.element(node).unwrap();
        let styles = elem.text_styles.as_ref().unwrap();
        assert_eq!(styles.color, Some(Color::Named(NamedColor::Red)));
    }

    #[test]
    fn test_text_widget_with_spans() {
        let mut tree = DomTree::new();
        let tw = TextWidget {
            content: "prefix ".into(),
            spans: vec![
                TextSpan {
                    content: "bold part".into(),
                    bold: true,
                    ..TextSpan::default()
                },
                TextSpan {
                    content: " italic part".into(),
                    italic: true,
                    ..TextSpan::default()
                },
            ],
            ..TextWidget::default()
        };
        let node = tw.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // 1 text node + 2 virtual text spans
        assert_eq!(elem.children.len(), 3);
    }

    #[test]
    fn test_text_widget_dimmed() {
        let mut tree = DomTree::new();
        let tw = TextWidget::dimmed("dim");
        let node = tw.build(&mut tree);
        let styles = tree.element(node).unwrap().text_styles.as_ref().unwrap();
        assert_eq!(styles.dim, Some(true));
    }
}
