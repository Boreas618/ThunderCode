//! Component layer for the ThunderCode terminal UI.
//!
//! Each widget produces a DOM subtree (via [`DomTree`]) that the layout
//! engine and renderer consume.  The [`Widget`] trait provides `build`
//! (full construction) and `update` (incremental patch) entry points.

pub mod app_layout;
pub mod box_widget;
pub mod dialog;
pub mod message_widgets;
pub mod pane;
pub mod permission_dialog;
pub mod prompt_input;
pub mod scroll_box;
pub mod spinner_widget;
pub mod status_line;
pub mod text_widget;
pub mod welcome_banner;

use crate::tui::dom::{DomTree, NodeId};

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------

/// Trait implemented by every UI component.
///
/// A widget is a *stateless description* -- it reads its own fields and
/// emits DOM nodes.  Mutable UI state (cursor position, scroll offset, ...)
/// lives in dedicated structs that are passed to the widget via its fields.
pub trait Widget {
    /// Build the full DOM subtree for this widget and return its root node.
    fn build(&self, tree: &mut DomTree) -> NodeId;

    /// Patch an existing subtree rooted at `node` to reflect the widget's
    /// current state.  The default implementation tears down `node` and
    /// rebuilds from scratch.
    fn update(&self, tree: &mut DomTree, node: NodeId) {
        // Default: remove all children, then rebuild.
        if let Some(elem) = tree.element(node) {
            let children: Vec<NodeId> = elem.children.clone();
            for child in children {
                tree.remove_child(node, child);
            }
        }
        // Re-build returns a *new* root.  We copy the children of that
        // root into `node` so callers keep the same NodeId handle.
        let fresh = self.build(tree);
        if let Some(fresh_elem) = tree.element(fresh) {
            let new_children: Vec<NodeId> = fresh_elem.children.clone();
            for child in new_children {
                tree.append_child(node, child);
            }
        }
        tree.mark_dirty(node);
    }
}

// ---------------------------------------------------------------------------
// Border character sets
// ---------------------------------------------------------------------------

/// Box-drawing character set used for bordered containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Single,
    Double,
    Round,
    Heavy,
    Ascii,
}

/// Resolved border characters for each of the 8 positions.
#[derive(Debug, Clone, Copy)]
pub struct BorderChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub left_t: char,
    pub right_t: char,
}

impl BorderStyle {
    /// Return the box-drawing character set for this style.
    pub fn chars(self) -> Option<BorderChars> {
        match self {
            BorderStyle::None => None,
            BorderStyle::Single => Some(BorderChars {
                top_left: '\u{250C}',     // ┌
                top_right: '\u{2510}',    // ┐
                bottom_left: '\u{2514}',  // └
                bottom_right: '\u{2518}', // ┘
                horizontal: '\u{2500}',   // ─
                vertical: '\u{2502}',     // │
                left_t: '\u{251C}',       // ├
                right_t: '\u{2524}',      // ┤
            }),
            BorderStyle::Double => Some(BorderChars {
                top_left: '\u{2554}',     // ╔
                top_right: '\u{2557}',    // ╗
                bottom_left: '\u{255A}',  // ╚
                bottom_right: '\u{255D}', // ╝
                horizontal: '\u{2550}',   // ═
                vertical: '\u{2551}',     // ║
                left_t: '\u{2560}',       // ╠
                right_t: '\u{2563}',      // ╣
            }),
            BorderStyle::Round => Some(BorderChars {
                top_left: '\u{256D}',     // ╭
                top_right: '\u{256E}',    // ╮
                bottom_left: '\u{2570}',  // ╰
                bottom_right: '\u{256F}', // ╯
                horizontal: '\u{2500}',   // ─
                vertical: '\u{2502}',     // │
                left_t: '\u{251C}',       // ├
                right_t: '\u{2524}',      // ┤
            }),
            BorderStyle::Heavy => Some(BorderChars {
                top_left: '\u{250F}',     // ┏
                top_right: '\u{2513}',    // ┓
                bottom_left: '\u{2517}',  // ┗
                bottom_right: '\u{251B}', // ┛
                horizontal: '\u{2501}',   // ━
                vertical: '\u{2503}',     // ┃
                left_t: '\u{2523}',       // ┣
                right_t: '\u{252B}',      // ┫
            }),
            BorderStyle::Ascii => Some(BorderChars {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                left_t: '+',
                right_t: '+',
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use app_layout::AppLayout;
pub use box_widget::BoxWidget;
pub use dialog::Dialog;
pub use message_widgets::{
    AssistantTextWidget, ThinkingWidget, ToolResultWidget, ToolUseWidget, UserMessageWidget,
};
pub use pane::Pane;
pub use permission_dialog::PermissionDialog;
pub use prompt_input::PromptInput;
pub use scroll_box::ScrollBox;
pub use spinner_widget::SpinnerWidget;
pub use status_line::StatusLine;
pub use text_widget::TextWidget;
pub use welcome_banner::WelcomeBanner;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dom::DomTree;
    use crate::tui::layout::ElementType;

    /// A trivial widget for testing the trait default `update` path.
    struct Dummy;

    impl Widget for Dummy {
        fn build(&self, tree: &mut DomTree) -> NodeId {
            let root = tree.create_element(ElementType::Box);
            let text = tree.create_text_node("hello");
            tree.append_child(root, text);
            root
        }
    }

    #[test]
    fn test_widget_build_and_update() {
        let mut tree = DomTree::new();
        let node = Dummy.build(&mut tree);
        assert_eq!(tree.element(node).unwrap().children.len(), 1);

        // update should rebuild children
        Dummy.update(&mut tree, node);
        assert_eq!(tree.element(node).unwrap().children.len(), 1);
    }

    #[test]
    fn test_border_chars() {
        assert!(BorderStyle::None.chars().is_none());
        let single = BorderStyle::Single.chars().unwrap();
        assert_eq!(single.horizontal, '\u{2500}');
        let round = BorderStyle::Round.chars().unwrap();
        assert_eq!(round.top_left, '\u{256D}');
        let heavy = BorderStyle::Heavy.chars().unwrap();
        assert_eq!(heavy.horizontal, '\u{2501}');
        let ascii = BorderStyle::Ascii.chars().unwrap();
        assert_eq!(ascii.top_left, '+');
        let double = BorderStyle::Double.chars().unwrap();
        assert_eq!(double.top_left, '\u{2554}');
    }
}
