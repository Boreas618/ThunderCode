//! Permission request dialog -- "Allow [tool]?" with y/n.
//!
//! Port of `ref/components/PermissionRequest.tsx`.  Shows a bordered
//! dialog with the tool name, description text, and y/n key hints.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::{Color, NamedColor};

use super::dialog::Dialog;
use super::text_widget::TextWidget;
use super::{BorderStyle, Widget};

/// Permission dialog state and configuration.
pub struct PermissionDialog {
    /// Tool name (e.g., "Bash", "Write").
    pub tool_name: String,
    /// Description / command text shown in the dialog body.
    pub description: String,
    /// Color of the dialog border.  Default: yellow (permission theme).
    pub border_color: Color,
    /// User response: `None` if pending, `Some(true)` for allow,
    /// `Some(false)` for deny.
    pub response: Option<bool>,
}

impl PermissionDialog {
    pub fn new(tool_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            description: description.into(),
            border_color: Color::Named(NamedColor::Yellow),
            response: None,
        }
    }

    /// Whether this dialog is still awaiting a response.
    pub fn is_pending(&self) -> bool {
        self.response.is_none()
    }

    /// Set the response.
    pub fn respond(&mut self, allow: bool) {
        self.response = Some(allow);
    }
}

impl Widget for PermissionDialog {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Build the inner content nodes.
        // 1. "Allow [tool_name]?" heading
        let heading = tree.create_element(ElementType::Box);
        tree.set_style(
            heading,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Row,
                margin: Edges {
                    bottom: 1.0,
                    ..Edges::default()
                },
                ..LayoutStyle::default()
            },
        );
        let heading_text = TextWidget {
            content: format!("Allow {}?", self.tool_name),
            bold: true,
            ..TextWidget::default()
        };
        let heading_node = heading_text.build(tree);
        tree.append_child(heading, heading_node);

        // 2. Description text (dimmed)
        let desc_node = if !self.description.is_empty() {
            let desc_tw = TextWidget::dimmed(&self.description);
            Some(desc_tw.build(tree))
        } else {
            None
        };

        // 3. Key hints row: [y] Allow  [n] Deny
        let hints_row = tree.create_element(ElementType::Box);
        tree.set_style(
            hints_row,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Row,
                margin: Edges {
                    top: 1.0,
                    ..Edges::default()
                },
                ..LayoutStyle::default()
            },
        );

        let y_key = TextWidget {
            content: "[y]".into(),
            bold: true,
            color: Some(Color::Named(NamedColor::Green)),
            ..TextWidget::default()
        };
        let y_node = y_key.build(tree);
        tree.append_child(hints_row, y_node);

        let y_label = TextWidget::plain(" Allow  ");
        let y_label_node = y_label.build(tree);
        tree.append_child(hints_row, y_label_node);

        let n_key = TextWidget {
            content: "[n]".into(),
            bold: true,
            color: Some(Color::Named(NamedColor::Red)),
            ..TextWidget::default()
        };
        let n_node = n_key.build(tree);
        tree.append_child(hints_row, n_node);

        let n_label = TextWidget::plain(" Deny");
        let n_label_node = n_label.build(tree);
        tree.append_child(hints_row, n_label_node);

        // Assemble children for the Dialog wrapper.
        let mut children = vec![heading];
        if let Some(dn) = desc_node {
            children.push(dn);
        }
        children.push(hints_row);

        // Wrap in a Dialog.
        let dialog = Dialog {
            border_style: BorderStyle::Round,
            border_color: Some(self.border_color.clone()),
            title: Some("Permission".into()),
            children,
            ..Dialog::default()
        };
        dialog.build(tree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_dialog_new() {
        let d = PermissionDialog::new("Bash", "rm -rf /tmp/test");
        assert!(d.is_pending());
        assert_eq!(d.tool_name, "Bash");
    }

    #[test]
    fn test_permission_dialog_respond() {
        let mut d = PermissionDialog::new("Write", "src/lib.rs");
        d.respond(true);
        assert!(!d.is_pending());
        assert_eq!(d.response, Some(true));
    }

    #[test]
    fn test_permission_dialog_build() {
        let mut tree = DomTree::new();
        let d = PermissionDialog::new("Bash", "echo hello");
        let node = d.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // Dialog should have children (heading, desc, hints)
        assert!(elem.children.len() >= 2);
    }
}
