//! Scrollable container widget -- Rust equivalent of Ink's `<ScrollBox>`.
//!
//! Wraps content in a `Box` with `overflow: scroll`, tracks scroll
//! position, and supports sticky-scroll (pin to bottom).

use crate::tui::dom::{DomNodeAttribute, DomTree, NodeId};
use crate::tui::layout::{
    Dimension, ElementType, LayoutFlexDirection, LayoutOverflow, LayoutStyle,
};

use super::Widget;

/// Scroll state for a `ScrollBox`.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Current scroll offset (pixels/rows from top).
    pub scroll_top: i32,
    /// Pending delta to be drained by the renderer.
    pub pending_scroll_delta: i32,
    /// Total content height (set by renderer after layout).
    pub scroll_height: i32,
    /// Visible viewport height (set by renderer after layout).
    pub viewport_height: i32,
    /// Whether scroll is pinned to the bottom.
    pub sticky: bool,
}

impl ScrollState {
    /// Scroll to an absolute position, breaking stickiness.
    pub fn scroll_to(&mut self, y: i32) {
        self.sticky = false;
        self.pending_scroll_delta = 0;
        self.scroll_top = y.max(0);
    }

    /// Scroll by a relative delta, breaking stickiness.
    pub fn scroll_by(&mut self, dy: i32) {
        self.sticky = false;
        self.pending_scroll_delta += dy;
    }

    /// Pin scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.sticky = true;
        self.pending_scroll_delta = 0;
    }

    /// Whether the view is currently at or near the bottom.
    pub fn is_at_bottom(&self) -> bool {
        self.sticky
            || (self.scroll_top + self.viewport_height >= self.scroll_height)
    }

    /// Apply pending delta, clamp, and drain.  Called by the renderer
    /// each frame before layout.
    pub fn drain(&mut self) {
        if self.sticky {
            // Pin to bottom
            self.scroll_top = (self.scroll_height - self.viewport_height).max(0);
            self.pending_scroll_delta = 0;
            return;
        }
        if self.pending_scroll_delta != 0 {
            self.scroll_top += self.pending_scroll_delta;
            self.pending_scroll_delta = 0;
        }
        let max_scroll = (self.scroll_height - self.viewport_height).max(0);
        self.scroll_top = self.scroll_top.clamp(0, max_scroll);
    }
}

/// A scrollable container with an imperative scroll API.
///
/// Port of `ref/ink/components/ScrollBox.tsx`.
#[derive(Debug, Clone)]
pub struct ScrollBox {
    /// Layout properties for the outer container.
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub width: Dimension,
    pub height: Dimension,
    pub flex_direction: LayoutFlexDirection,

    /// Whether to auto-pin to bottom when content grows.
    pub sticky_scroll: bool,

    /// Pre-built child node IDs.
    pub children: Vec<NodeId>,
}

impl Default for ScrollBox {
    fn default() -> Self {
        Self {
            flex_grow: 0.0,
            flex_shrink: 1.0,
            width: Dimension::Auto,
            height: Dimension::Auto,
            flex_direction: LayoutFlexDirection::Column,
            sticky_scroll: false,
            children: Vec::new(),
        }
    }
}

impl Widget for ScrollBox {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Outer element: constrained container with overflow: scroll.
        let node = tree.create_element(ElementType::Box);
        let style = LayoutStyle {
            flex_direction: self.flex_direction,
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            overflow: LayoutOverflow::Scroll,
            width: self.width,
            height: self.height,
            ..LayoutStyle::default()
        };
        tree.set_style(node, style);

        if self.sticky_scroll {
            tree.set_attribute(
                node,
                "stickyScroll",
                DomNodeAttribute::Bool(true),
            );
        }

        // Inner wrapper: full-height content container.
        let inner = tree.create_element(ElementType::Box);
        let inner_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            flex_shrink: 0.0,
            width: Dimension::Percent(100.0),
            ..LayoutStyle::default()
        };
        tree.set_style(inner, inner_style);

        for &child_id in &self.children {
            tree.append_child(inner, child_id);
        }

        tree.append_child(node, inner);
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_default() {
        let s = ScrollState::default();
        assert_eq!(s.scroll_top, 0);
        assert!(!s.sticky);
    }

    #[test]
    fn test_scroll_to_bottom() {
        let mut s = ScrollState {
            scroll_height: 100,
            viewport_height: 20,
            ..Default::default()
        };
        s.scroll_to_bottom();
        assert!(s.sticky);
        s.drain();
        assert_eq!(s.scroll_top, 80);
    }

    #[test]
    fn test_scroll_by() {
        let mut s = ScrollState {
            scroll_height: 100,
            viewport_height: 20,
            scroll_top: 10,
            ..Default::default()
        };
        s.scroll_by(5);
        assert!(!s.sticky);
        s.drain();
        assert_eq!(s.scroll_top, 15);
    }

    #[test]
    fn test_scroll_clamp() {
        let mut s = ScrollState {
            scroll_height: 50,
            viewport_height: 20,
            scroll_top: 0,
            ..Default::default()
        };
        s.scroll_by(999);
        s.drain();
        assert_eq!(s.scroll_top, 30); // max = 50 - 20
    }

    #[test]
    fn test_scroll_box_build() {
        let mut tree = DomTree::new();
        let child = tree.create_text_node("content");
        let sb = ScrollBox {
            sticky_scroll: true,
            children: vec![child],
            flex_grow: 1.0,
            ..Default::default()
        };
        let node = sb.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // Should have inner wrapper as child
        assert_eq!(elem.children.len(), 1);
    }
}
