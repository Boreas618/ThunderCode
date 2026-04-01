//! Box (flex container) widget -- the Rust equivalent of Ink's `<Box>`.
//!
//! Produces a DOM `Box` element with flexbox layout properties, optional
//! border, and optional background color.

use crate::tui::dom::{DomNodeAttribute, DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutDisplay, LayoutFlexDirection, LayoutOverflow, LayoutStyle,
};
use crate::tui::style::{Color, TextStyles};

use super::{BorderStyle, Widget};

/// A flex container with layout props, optional borders, and background.
///
/// Port of `ref/ink/components/Box.tsx`.
#[derive(Debug, Clone)]
pub struct BoxWidget {
    // -- Flex layout --
    pub flex_direction: LayoutFlexDirection,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub flex_wrap: taffy::FlexWrap,
    pub align_items: Option<taffy::AlignItems>,
    pub align_self: Option<taffy::AlignSelf>,
    pub justify_content: Option<taffy::JustifyContent>,
    pub overflow: LayoutOverflow,
    pub display: LayoutDisplay,

    // -- Sizing --
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    // -- Spacing --
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub column_gap: Option<f32>,
    pub row_gap: Option<f32>,

    // -- Border --
    pub border_style: BorderStyle,
    pub border_color: Option<Color>,

    // -- Background --
    pub background_color: Option<Color>,

    // -- Children --
    /// Pre-built child node IDs (already present in the same `DomTree`).
    pub children: Vec<NodeId>,
}

impl Default for BoxWidget {
    fn default() -> Self {
        Self {
            flex_direction: LayoutFlexDirection::Row,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            flex_wrap: taffy::FlexWrap::NoWrap,
            align_items: None,
            align_self: None,
            justify_content: None,
            overflow: LayoutOverflow::Visible,
            display: LayoutDisplay::Flex,
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_width: Dimension::Auto,
            min_height: Dimension::Auto,
            max_width: Dimension::Auto,
            max_height: Dimension::Auto,
            padding: Edges::default(),
            margin: Edges::default(),
            gap: 0.0,
            column_gap: None,
            row_gap: None,
            border_style: BorderStyle::None,
            border_color: None,
            background_color: None,
            children: Vec::new(),
        }
    }
}

impl BoxWidget {
    /// Convenience: create a column layout container.
    pub fn column() -> Self {
        Self {
            flex_direction: LayoutFlexDirection::Column,
            ..Default::default()
        }
    }

    /// Convenience: create a row layout container.
    pub fn row() -> Self {
        Self::default()
    }

    /// Convenience: build the layout style.
    fn to_layout_style(&self) -> LayoutStyle {
        let border_width = if self.border_style != BorderStyle::None {
            1.0
        } else {
            0.0
        };
        LayoutStyle {
            display: self.display,
            flex_direction: self.flex_direction,
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            flex_basis: self.flex_basis,
            align_items: self.align_items,
            align_self: self.align_self,
            justify_content: self.justify_content,
            flex_wrap: self.flex_wrap,
            overflow: self.overflow,
            width: self.width,
            height: self.height,
            min_width: self.min_width,
            min_height: self.min_height,
            max_width: self.max_width,
            max_height: self.max_height,
            padding: self.padding,
            margin: self.margin,
            border: Edges {
                top: border_width,
                bottom: border_width,
                left: border_width,
                right: border_width,
            },
            gap: self.gap,
            column_gap: self.column_gap,
            row_gap: self.row_gap,
            ..LayoutStyle::default()
        }
    }
}

impl Widget for BoxWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let node = tree.create_element(ElementType::Box);
        tree.set_style(node, self.to_layout_style());

        // Store border style as attribute so the renderer can draw borders.
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
                node,
                "borderStyle",
                DomNodeAttribute::String(style_name.into()),
            );
        }

        if let Some(ref color) = self.border_color {
            tree.set_attribute(
                node,
                "borderColor",
                DomNodeAttribute::String(format!("{:?}", color)),
            );
        }

        if let Some(ref bg) = self.background_color {
            let styles = TextStyles {
                background_color: Some(bg.clone()),
                ..TextStyles::default()
            };
            tree.set_text_styles(node, styles);
        }

        // Append pre-built children.
        for &child_id in &self.children {
            tree.append_child(node, child_id);
        }

        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_widget_default() {
        let b = BoxWidget::default();
        assert_eq!(b.flex_direction, LayoutFlexDirection::Row);
        assert_eq!(b.flex_shrink, 1.0);
        assert_eq!(b.border_style, BorderStyle::None);
    }

    #[test]
    fn test_box_widget_column() {
        let b = BoxWidget::column();
        assert_eq!(b.flex_direction, LayoutFlexDirection::Column);
    }

    #[test]
    fn test_box_widget_build() {
        let mut tree = DomTree::new();
        let child = tree.create_text_node("hi");
        let bw = BoxWidget {
            children: vec![child],
            border_style: BorderStyle::Round,
            ..BoxWidget::column()
        };
        let node = bw.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        assert_eq!(elem.children.len(), 1);
        assert!(elem.attributes.contains_key("borderStyle"));
    }

    #[test]
    fn test_box_widget_with_background() {
        let mut tree = DomTree::new();
        let bw = BoxWidget {
            background_color: Some(Color::Rgb(30, 30, 30)),
            ..BoxWidget::default()
        };
        let node = bw.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert!(elem.text_styles.is_some());
    }
}
