//! Flexbox layout engine wrapping `taffy`.
//!
//! Provides a Yoga-compatible layout interface for the DOM tree.
//! Layout properties are translated to taffy's CSS flexbox model.

use taffy::prelude::*;
use taffy::Overflow;

use crate::tui::text::TextWrap;

/// Layout data attached to each taffy node.
#[derive(Debug, Default)]
pub struct LayoutData {
    /// The type of DOM element this node represents.
    pub element_type: ElementType,
    /// For Text / RawAnsi leaf nodes, the text content used for measurement.
    pub text_content: Option<String>,
}

/// DOM element types that participate in layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementType {
    #[default]
    Root,
    Box,
    Text,
    VirtualText,
    Link,
    Progress,
    RawAnsi,
}

impl ElementType {
    /// Whether this element type has a yoga/taffy layout node.
    pub fn needs_layout_node(&self) -> bool {
        !matches!(self, Self::VirtualText | Self::Link | Self::Progress)
    }
}

/// Flexbox layout engine.
pub struct LayoutEngine {
    pub taffy: TaffyTree<LayoutData>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
        }
    }

    /// Create a new layout node with default style.
    pub fn new_leaf(&mut self, element_type: ElementType) -> NodeId {
        let data = LayoutData { element_type, text_content: None };
        self.taffy
            .new_leaf_with_context(Style::default(), data)
            .expect("failed to create layout node")
    }

    /// Create a new layout node with a measure function.
    pub fn new_leaf_with_measure(
        &mut self,
        element_type: ElementType,
        _measure: MeasureFunc,
    ) -> NodeId {
        let data = LayoutData { element_type, text_content: None };
        self.taffy
            .new_leaf_with_context(Style::default(), data)
            .expect("failed to create measured layout node")
    }

    /// Create a new container node with children.
    pub fn new_with_children(
        &mut self,
        element_type: ElementType,
        style: Style,
        children: &[NodeId],
    ) -> NodeId {
        let data = LayoutData { element_type, text_content: None };
        let node = self
            .taffy
            .new_with_children(style, children)
            .expect("failed to create container node");
        self.taffy
            .set_node_context(node, Some(data))
            .expect("failed to set node context");
        node
    }

    /// Set the style of a node.
    pub fn set_style(&mut self, node: NodeId, style: Style) {
        self.taffy.set_style(node, style).ok();
    }

    /// Get the computed layout for a node.
    pub fn layout(&self, node: NodeId) -> &taffy::Layout {
        self.taffy.layout(node).expect("node has no layout")
    }

    /// Add a child to a parent node.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.taffy.add_child(parent, child).ok();
    }

    /// Remove a child from a parent node.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        self.taffy.remove_child(parent, child).ok();
    }

    /// Remove all children from a parent node.
    pub fn remove_children(&mut self, parent: NodeId) {
        // Use set_children with empty slice to clear children
        self.taffy.set_children(parent, &[]).ok();
    }

    /// Compute layout for the tree rooted at `node` with the given available space.
    pub fn compute_layout(&mut self, node: NodeId, available_width: f32, available_height: f32) {
        let available_space = Size {
            width: AvailableSpace::Definite(available_width),
            height: AvailableSpace::Definite(available_height),
        };
        self.taffy
            .compute_layout_with_measure(
                node,
                available_space,
                |known, available, _node_id, context, _style| {
                    // For text nodes with content, compute wrapped dimensions
                    if let Some(data) = context {
                        if let Some(ref text) = data.text_content {
                            if !text.is_empty() {
                                let avail_w = known.width.unwrap_or_else(|| match available.width {
                                    AvailableSpace::Definite(w) => w,
                                    AvailableSpace::MinContent => 0.0,
                                    AvailableSpace::MaxContent => f32::MAX,
                                });
                                let max_w = if avail_w <= 0.0 || avail_w >= f32::MAX {
                                    usize::MAX
                                } else {
                                    avail_w as usize
                                };
                                let (text_w, text_h) = crate::tui::text::measure_text(text, max_w);
                                return Size {
                                    width: known.width.unwrap_or(text_w as f32),
                                    height: known.height.unwrap_or(text_h as f32),
                                };
                            }
                        }
                    }
                    // Fallback: use known size or available size
                    Size {
                        width: known
                            .width
                            .unwrap_or_else(|| match available.width {
                                AvailableSpace::Definite(w) => w,
                                _ => 0.0,
                            }),
                        height: known
                            .height
                            .unwrap_or_else(|| match available.height {
                                AvailableSpace::Definite(h) => h,
                                _ => 0.0,
                            }),
                    }
                },
            )
            .ok();
    }

    /// Mark a node as dirty (needs re-layout).
    pub fn mark_dirty(&mut self, node: NodeId) {
        self.taffy.mark_dirty(node).ok();
    }

    /// Get the number of children of a node.
    pub fn child_count(&self, node: NodeId) -> usize {
        self.taffy.child_count(node)
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Layout style properties matching the ref's CSS flexbox subset.
/// Used to configure DOM elements before translating to taffy `Style`.
#[derive(Debug, Clone, Default)]
pub struct LayoutStyle {
    pub display: LayoutDisplay,
    pub flex_direction: LayoutFlexDirection,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub justify_content: Option<JustifyContent>,
    pub flex_wrap: FlexWrap,
    pub overflow: LayoutOverflow,
    pub position: LayoutPosition,

    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    pub padding: Edges,
    pub margin: Edges,
    pub border: Edges,

    pub gap: f32,
    pub column_gap: Option<f32>,
    pub row_gap: Option<f32>,

    pub top: Dimension,
    pub bottom: Dimension,
    pub left: Dimension,
    pub right: Dimension,

    pub text_wrap: TextWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDisplay {
    #[default]
    Flex,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutFlexDirection {
    Row,
    RowReverse,
    #[default]
    Column,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutOverflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutPosition {
    #[default]
    Relative,
    Absolute,
}

/// Edge values for padding, margin, border.
#[derive(Debug, Clone, Copy, Default)]
pub struct Edges {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// Dimension that can be auto, points, or percent.
#[derive(Debug, Clone, Copy, Default)]
pub enum Dimension {
    #[default]
    Auto,
    Points(f32),
    Percent(f32),
}

impl LayoutStyle {
    /// Convert to a taffy `Style`.
    pub fn to_taffy_style(&self) -> Style {
        let mut style = Style::default();

        style.display = match self.display {
            LayoutDisplay::Flex => Display::Flex,
            LayoutDisplay::None => Display::None,
        };

        style.flex_direction = match self.flex_direction {
            LayoutFlexDirection::Row => FlexDirection::Row,
            LayoutFlexDirection::RowReverse => FlexDirection::RowReverse,
            LayoutFlexDirection::Column => FlexDirection::Column,
            LayoutFlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
        };

        style.flex_grow = self.flex_grow;
        style.flex_shrink = self.flex_shrink;

        style.flex_basis = dim_to_taffy(self.flex_basis);

        if let Some(ai) = self.align_items {
            style.align_items = Some(ai);
        }
        if let Some(als) = self.align_self {
            style.align_self = Some(als);
        }
        if let Some(jc) = self.justify_content {
            style.justify_content = Some(jc);
        }

        style.flex_wrap = self.flex_wrap;

        style.overflow = taffy::Point {
            x: match self.overflow {
                LayoutOverflow::Visible => Overflow::Visible,
                LayoutOverflow::Hidden => Overflow::Hidden,
                LayoutOverflow::Scroll => Overflow::Scroll,
            },
            y: match self.overflow {
                LayoutOverflow::Visible => Overflow::Visible,
                LayoutOverflow::Hidden => Overflow::Hidden,
                LayoutOverflow::Scroll => Overflow::Scroll,
            },
        };

        style.position = match self.position {
            LayoutPosition::Relative => Position::Relative,
            LayoutPosition::Absolute => Position::Absolute,
        };

        style.size = Size {
            width: dim_to_taffy(self.width),
            height: dim_to_taffy(self.height),
        };
        style.min_size = Size {
            width: dim_to_taffy(self.min_width),
            height: dim_to_taffy(self.min_height),
        };
        style.max_size = Size {
            width: dim_to_taffy(self.max_width),
            height: dim_to_taffy(self.max_height),
        };

        style.padding = taffy::Rect {
            top: length(self.padding.top),
            bottom: length(self.padding.bottom),
            left: length(self.padding.left),
            right: length(self.padding.right),
        };
        style.margin = taffy::Rect {
            top: length_auto(self.margin.top),
            bottom: length_auto(self.margin.bottom),
            left: length_auto(self.margin.left),
            right: length_auto(self.margin.right),
        };
        style.border = taffy::Rect {
            top: length(self.border.top),
            bottom: length(self.border.bottom),
            left: length(self.border.left),
            right: length(self.border.right),
        };

        let cg = self.column_gap.unwrap_or(self.gap);
        let rg = self.row_gap.unwrap_or(self.gap);
        style.gap = Size {
            width: LengthPercentage::Length(cg),
            height: LengthPercentage::Length(rg),
        };

        style.inset = taffy::Rect {
            top: dim_to_auto(self.top),
            bottom: dim_to_auto(self.bottom),
            left: dim_to_auto(self.left),
            right: dim_to_auto(self.right),
        };

        style
    }
}

fn dim_to_taffy(dim: Dimension) -> taffy::Dimension {
    match dim {
        Dimension::Auto => taffy::Dimension::Auto,
        Dimension::Points(v) => taffy::Dimension::Length(v),
        Dimension::Percent(v) => taffy::Dimension::Percent(v / 100.0),
    }
}

fn dim_to_auto(dim: Dimension) -> LengthPercentageAuto {
    match dim {
        Dimension::Auto => LengthPercentageAuto::Auto,
        Dimension::Points(v) => LengthPercentageAuto::Length(v),
        Dimension::Percent(v) => LengthPercentageAuto::Percent(v / 100.0),
    }
}

fn length(v: f32) -> LengthPercentage {
    LengthPercentage::Length(v)
}

fn length_auto(v: f32) -> LengthPercentageAuto {
    if v == 0.0 {
        LengthPercentageAuto::Length(0.0)
    } else {
        LengthPercentageAuto::Length(v)
    }
}

/// Measure function type that computes text dimensions.
pub type MeasureFunc = Box<dyn Fn(f32) -> (f32, f32) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_engine_basic() {
        let mut engine = LayoutEngine::new();
        let root = engine.new_leaf(ElementType::Root);
        engine.set_style(
            root,
            Style {
                size: Size {
                    width: taffy::Dimension::Length(80.0),
                    height: taffy::Dimension::Length(24.0),
                },
                ..Default::default()
            },
        );
        engine.compute_layout(root, 80.0, 24.0);
        let layout = engine.layout(root);
        assert_eq!(layout.size.width, 80.0);
        assert_eq!(layout.size.height, 24.0);
    }

    #[test]
    fn test_layout_style_conversion() {
        let ls = LayoutStyle {
            display: LayoutDisplay::Flex,
            flex_direction: LayoutFlexDirection::Row,
            flex_grow: 1.0,
            width: Dimension::Points(100.0),
            height: Dimension::Percent(50.0),
            padding: Edges {
                top: 1.0,
                bottom: 1.0,
                left: 2.0,
                right: 2.0,
            },
            ..Default::default()
        };
        let ts = ls.to_taffy_style();
        assert_eq!(ts.flex_direction, FlexDirection::Row);
        assert_eq!(ts.flex_grow, 1.0);
    }

    #[test]
    fn test_layout_children() {
        let mut engine = LayoutEngine::new();
        let root = engine.new_leaf(ElementType::Root);
        let child = engine.new_leaf(ElementType::Box);

        engine.set_style(
            root,
            Style {
                size: Size {
                    width: taffy::Dimension::Length(80.0),
                    height: taffy::Dimension::Length(24.0),
                },
                ..Default::default()
            },
        );

        engine.set_style(
            child,
            Style {
                size: Size {
                    width: taffy::Dimension::Length(40.0),
                    height: taffy::Dimension::Length(10.0),
                },
                ..Default::default()
            },
        );

        engine.add_child(root, child);
        assert_eq!(engine.child_count(root), 1);

        engine.compute_layout(root, 80.0, 24.0);
        let child_layout = engine.layout(child);
        assert_eq!(child_layout.size.width, 40.0);
        assert_eq!(child_layout.size.height, 10.0);
    }

    #[test]
    fn test_element_type_needs_layout() {
        assert!(ElementType::Root.needs_layout_node());
        assert!(ElementType::Box.needs_layout_node());
        assert!(ElementType::Text.needs_layout_node());
        assert!(!ElementType::VirtualText.needs_layout_node());
        assert!(!ElementType::Link.needs_layout_node());
        assert!(!ElementType::Progress.needs_layout_node());
    }
}
