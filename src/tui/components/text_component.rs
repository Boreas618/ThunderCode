//! Text component -- styled text rendering with color, bold, dim, etc.
//!
//! Mirrors the ref's `<Text>` component. Creates a text-type DOM element
//! with TextStyles applied and a child text node containing the content.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{ElementType, LayoutStyle};
use crate::tui::style::{Color, TextStyles};

/// Configuration for creating a styled text element.
#[derive(Debug, Clone, Default)]
pub struct TextComponent {
    pub content: String,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub inverse: Option<bool>,
}

impl TextComponent {
    /// Create a new text component with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Self::default()
        }
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set bold.
    pub fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }

    /// Set dim.
    pub fn dim(mut self) -> Self {
        self.dim = Some(true);
        self
    }

    /// Set italic.
    pub fn italic(mut self) -> Self {
        self.italic = Some(true);
        self
    }

    /// Set underline.
    pub fn underline(mut self) -> Self {
        self.underline = Some(true);
        self
    }

    /// Set inverse.
    pub fn inverse(mut self) -> Self {
        self.inverse = Some(true);
        self
    }

    /// Build this text as a DOM element with a child text node, returning the element NodeId.
    pub fn build(&self, tree: &mut DomTree) -> NodeId {
        let elem_id = tree.create_element(ElementType::Text);

        let styles = TextStyles {
            color: self.color.clone(),
            background_color: self.background_color.clone(),
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            inverse: self.inverse,
        };
        tree.set_text_styles(elem_id, styles);

        // Create child text node
        let text_id = tree.create_text_node(&self.content);
        tree.append_child(elem_id, text_id);

        elem_id
    }

    /// Convert TextStyles to the style pool's AnsiCode representation.
    pub fn to_text_styles(&self) -> TextStyles {
        TextStyles {
            color: self.color.clone(),
            background_color: self.background_color.clone(),
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            inverse: self.inverse,
        }
    }
}
