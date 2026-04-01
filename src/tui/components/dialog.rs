//! Dialog widget -- a bordered overlay for prompts and confirmations.
//!
//! Port of `ref/components/permissions/PermissionRequest.tsx` and similar
//! modal overlays.

use crate::tui::dom::{DomNodeAttribute, DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::{Color, TextStyles};

use super::{BorderStyle, Widget};

/// A bordered dialog for displaying prompts, confirmations, etc.
pub struct Dialog {
    /// Border style (default: Round).
    pub border_style: BorderStyle,
    /// Border color.
    pub border_color: Option<Color>,
    /// Optional title text displayed in the top border.
    pub title: Option<String>,
    /// Width of the dialog. Default = 100%.
    pub width: Dimension,
    /// Pre-built content children.
    pub children: Vec<NodeId>,
}

impl Default for Dialog {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Round,
            border_color: None,
            title: None,
            width: Dimension::Percent(100.0),
            children: Vec::new(),
        }
    }
}

impl Widget for Dialog {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let node = tree.create_element(ElementType::Box);

        let style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: self.width,
            padding: Edges {
                top: 0.0,
                bottom: 0.0,
                left: 1.0,
                right: 1.0,
            },
            border: Edges {
                top: 1.0,
                bottom: 1.0,
                left: 1.0,
                right: 1.0,
            },
            ..LayoutStyle::default()
        };
        tree.set_style(node, style);

        // Store border attributes
        let style_name = match self.border_style {
            BorderStyle::Single => "single",
            BorderStyle::Double => "double",
            BorderStyle::Round => "round",
            BorderStyle::Heavy => "heavy",
            BorderStyle::Ascii => "ascii",
            BorderStyle::None => "none",
        };
        tree.set_attribute(
            node,
            "borderStyle",
            DomNodeAttribute::String(style_name.into()),
        );

        if let Some(ref color) = self.border_color {
            tree.set_attribute(
                node,
                "borderColor",
                DomNodeAttribute::String(format!("{:?}", color)),
            );
            tree.set_text_styles(
                node,
                TextStyles {
                    color: Some(color.clone()),
                    ..TextStyles::default()
                },
            );
        }

        if let Some(ref title) = self.title {
            tree.set_attribute(
                node,
                "borderTop",
                DomNodeAttribute::String(title.clone()),
            );
        }

        // Append children
        for &child_id in &self.children {
            tree.append_child(node, child_id);
        }

        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::NamedColor;

    #[test]
    fn test_dialog_build() {
        let mut tree = DomTree::new();
        let text = tree.create_text_node("Allow bash?");
        let dialog = Dialog {
            border_color: Some(Color::Named(NamedColor::Yellow)),
            title: Some("Permission".into()),
            children: vec![text],
            ..Dialog::default()
        };
        let node = dialog.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        assert_eq!(elem.children.len(), 1);
        assert!(elem.attributes.contains_key("borderStyle"));
        assert!(elem.attributes.contains_key("borderTop"));
    }
}
