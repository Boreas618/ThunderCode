//! Box component -- a flex container with optional borders, padding, colors.
//!
//! Mirrors the ref's `<Box>` component. Creates a DOM element with layout
//! styles that map to taffy flexbox properties.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutDisplay, LayoutFlexDirection, LayoutOverflow,
    LayoutStyle,
};
use crate::tui::style::{Color, TextStyles};

/// Border style for a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    Single,
    Double,
    Round,
    Bold,
    None,
}

/// Configuration for creating a Box element in the DOM.
#[derive(Debug, Clone)]
pub struct BoxComponent {
    pub flex_direction: LayoutFlexDirection,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub width: Dimension,
    pub height: Dimension,
    pub min_height: Dimension,
    pub max_height: Dimension,
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub border_style: BorderStyle,
    pub border_color: Option<Color>,
    pub overflow: LayoutOverflow,
    pub display: LayoutDisplay,
    pub align_items: Option<taffy::AlignItems>,
    pub justify_content: Option<taffy::JustifyContent>,
}

impl Default for BoxComponent {
    fn default() -> Self {
        Self {
            flex_direction: LayoutFlexDirection::Column,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_height: Dimension::Auto,
            max_height: Dimension::Auto,
            padding: Edges::default(),
            margin: Edges::default(),
            gap: 0.0,
            border_style: BorderStyle::None,
            border_color: None,
            overflow: LayoutOverflow::Visible,
            display: LayoutDisplay::Flex,
            align_items: None,
            justify_content: None,
        }
    }
}

impl BoxComponent {
    /// Create a new box with column direction (default).
    pub fn column() -> Self {
        Self::default()
    }

    /// Create a new box with row direction.
    pub fn row() -> Self {
        Self {
            flex_direction: LayoutFlexDirection::Row,
            ..Self::default()
        }
    }

    /// Set flex_grow.
    pub fn grow(mut self, v: f32) -> Self {
        self.flex_grow = v;
        self
    }

    /// Set width to 100%.
    pub fn full_width(mut self) -> Self {
        self.width = Dimension::Percent(100.0);
        self
    }

    /// Set fixed width.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Dimension::Points(w);
        self
    }

    /// Set fixed height.
    pub fn height(mut self, h: f32) -> Self {
        self.height = Dimension::Points(h);
        self
    }

    /// Set padding on all sides.
    pub fn padding_all(mut self, v: f32) -> Self {
        self.padding = Edges {
            top: v,
            bottom: v,
            left: v,
            right: v,
        };
        self
    }

    /// Set horizontal padding.
    pub fn padding_x(mut self, v: f32) -> Self {
        self.padding.left = v;
        self.padding.right = v;
        self
    }

    /// Set vertical padding.
    pub fn padding_y(mut self, v: f32) -> Self {
        self.padding.top = v;
        self.padding.bottom = v;
        self
    }

    /// Set margin top.
    pub fn margin_top(mut self, v: f32) -> Self {
        self.margin.top = v;
        self
    }

    /// Set margin bottom.
    pub fn margin_bottom(mut self, v: f32) -> Self {
        self.margin.bottom = v;
        self
    }

    /// Set gap between children.
    pub fn gap(mut self, v: f32) -> Self {
        self.gap = v;
        self
    }

    /// Set border style.
    pub fn border(mut self, style: BorderStyle) -> Self {
        self.border_style = style;
        self
    }

    /// Set border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Set overflow to hidden.
    pub fn overflow_hidden(mut self) -> Self {
        self.overflow = LayoutOverflow::Hidden;
        self
    }

    /// Set overflow to scroll.
    pub fn overflow_scroll(mut self) -> Self {
        self.overflow = LayoutOverflow::Scroll;
        self
    }

    /// Set align_items center.
    pub fn align_center(mut self) -> Self {
        self.align_items = Some(taffy::AlignItems::Center);
        self
    }

    /// Build this box as a DOM element, returning its NodeId.
    pub fn build(&self, tree: &mut DomTree) -> NodeId {
        let id = tree.create_element(ElementType::Box);

        let border_width = if self.border_style != BorderStyle::None {
            1.0
        } else {
            0.0
        };

        let style = LayoutStyle {
            display: self.display,
            flex_direction: self.flex_direction,
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            width: self.width,
            height: self.height,
            min_height: self.min_height,
            max_height: self.max_height,
            padding: self.padding,
            margin: self.margin,
            gap: self.gap,
            overflow: self.overflow,
            align_items: self.align_items,
            justify_content: self.justify_content,
            border: Edges {
                top: border_width,
                bottom: border_width,
                left: border_width,
                right: border_width,
            },
            ..LayoutStyle::default()
        };

        tree.set_style(id, style);

        // Store border info as attributes for the renderer
        if self.border_style != BorderStyle::None {
            tree.set_attribute(
                id,
                "borderStyle",
                crate::tui::dom::DomNodeAttribute::String(match self.border_style {
                    BorderStyle::Single => "single".into(),
                    BorderStyle::Double => "double".into(),
                    BorderStyle::Round => "round".into(),
                    BorderStyle::Bold => "bold".into(),
                    BorderStyle::None => "none".into(),
                }),
            );
        }

        id
    }
}

/// Border character sets for different border styles.
pub struct BorderChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

impl BorderChars {
    pub fn for_style(style: BorderStyle) -> Option<Self> {
        match style {
            BorderStyle::None => None,
            BorderStyle::Single => Some(Self {
                top_left: '\u{250c}',
                top_right: '\u{2510}',
                bottom_left: '\u{2514}',
                bottom_right: '\u{2518}',
                horizontal: '\u{2500}',
                vertical: '\u{2502}',
            }),
            BorderStyle::Double => Some(Self {
                top_left: '\u{2554}',
                top_right: '\u{2557}',
                bottom_left: '\u{255a}',
                bottom_right: '\u{255d}',
                horizontal: '\u{2550}',
                vertical: '\u{2551}',
            }),
            BorderStyle::Round => Some(Self {
                top_left: '\u{256d}',
                top_right: '\u{256e}',
                bottom_left: '\u{2570}',
                bottom_right: '\u{256f}',
                horizontal: '\u{2500}',
                vertical: '\u{2502}',
            }),
            BorderStyle::Bold => Some(Self {
                top_left: '\u{250f}',
                top_right: '\u{2513}',
                bottom_left: '\u{2517}',
                bottom_right: '\u{251b}',
                horizontal: '\u{2501}',
                vertical: '\u{2503}',
            }),
        }
    }
}
