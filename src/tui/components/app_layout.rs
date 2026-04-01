//! Main application layout that composes all components.
//!
//! Layout:
//! ```text
//! +--------------------------------------+
//! |  Messages (scrollable)               |
//! +--------------------------------------+
//! |  [Spinner / PermissionDialog]        |
//! +--------------------------------------+
//! |  PromptInput                         |
//! +--------------------------------------+
//! |  StatusLine                          |
//! +--------------------------------------+
//! ```

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutOverflow, LayoutStyle,
};

use super::Widget;

/// The full app layout.
pub struct AppLayout {
    pub show_welcome: bool,
    pub show_spinner: bool,
    pub show_permission_dialog: bool,
    pub terminal_width: u16,
    pub terminal_height: u16,
}

impl Default for AppLayout {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_spinner: false,
            show_permission_dialog: false,
            terminal_width: 80,
            terminal_height: 24,
        }
    }
}

impl Widget for AppLayout {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Root container: column, full size
        let root = tree.create_element(ElementType::Root);
        tree.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(self.terminal_width as f32),
                height: Dimension::Points(self.terminal_height as f32),
                ..LayoutStyle::default()
            },
        );

        // Messages area: flex-grow=1, overflow=hidden (rendered with scroll)
        let messages_area = tree.create_element(ElementType::Box);
        tree.set_style(
            messages_area,
            LayoutStyle {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                overflow: LayoutOverflow::Hidden,
                ..LayoutStyle::default()
            },
        );
        tree.append_child(root, messages_area);

        // Middle area: spinner or permission dialog (fixed height when visible)
        if self.show_spinner || self.show_permission_dialog {
            let middle = tree.create_element(ElementType::Box);
            tree.set_style(
                middle,
                LayoutStyle {
                    flex_shrink: 0.0,
                    padding: Edges {
                        top: 1.0,
                        bottom: 0.0,
                        left: 0.0,
                        right: 0.0,
                    },
                    ..LayoutStyle::default()
                },
            );
            tree.append_child(root, middle);
        }

        // Prompt input area: fixed height
        let prompt_area = tree.create_element(ElementType::Box);
        tree.set_style(
            prompt_area,
            LayoutStyle {
                flex_shrink: 0.0,
                min_height: Dimension::Points(1.0),
                ..LayoutStyle::default()
            },
        );
        tree.append_child(root, prompt_area);

        // Status line: fixed height = 1 row
        let status_area = tree.create_element(ElementType::Box);
        tree.set_style(
            status_area,
            LayoutStyle {
                flex_shrink: 0.0,
                height: Dimension::Points(1.0),
                ..LayoutStyle::default()
            },
        );
        tree.append_child(root, status_area);

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_layout_build() {
        let mut tree = DomTree::new();
        let layout = AppLayout::default();
        let root = layout.build(&mut tree);
        let elem = tree.element(root).unwrap();
        // messages + prompt + status = 3 children (no spinner by default)
        assert_eq!(elem.children.len(), 3);
    }

    #[test]
    fn test_app_layout_with_spinner() {
        let mut tree = DomTree::new();
        let layout = AppLayout {
            show_spinner: true,
            ..AppLayout::default()
        };
        let root = layout.build(&mut tree);
        let elem = tree.element(root).unwrap();
        // messages + middle(spinner) + prompt + status = 4 children
        assert_eq!(elem.children.len(), 4);
    }
}
