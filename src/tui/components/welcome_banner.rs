//! Welcome banner shown at startup.
//!
//! Displays the ThunderCode logo, version, model info, and help hints.
//! Mirrors the ref's `LogoV2` component.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::{Color, NamedColor};

use super::text_widget::TextWidget;
use super::Widget;

/// Welcome banner widget.
pub struct WelcomeBanner {
    /// Model name displayed in the banner.
    pub model: String,
    /// Number of registered tools.
    pub tool_count: usize,
    /// Number of available commands.
    pub command_count: usize,
    /// Application version string.
    pub version: String,
}

impl Widget for WelcomeBanner {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        let outer = tree.create_element(ElementType::Box);
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            gap: 0.0,
            padding: Edges {
                top: 1.0,
                bottom: 1.0,
                left: 2.0,
                right: 2.0,
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Title line: "ThunderCode v0.1.0"
        let title_row = tree.create_element(ElementType::Box);
        tree.set_style(
            title_row,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Row,
                ..LayoutStyle::default()
            },
        );

        let name_text = TextWidget {
            content: "ThunderCode".into(),
            color: Some(Color::Named(NamedColor::Cyan)),
            bold: true,
            ..TextWidget::default()
        };
        let name_node = name_text.build(tree);
        tree.append_child(title_row, name_node);

        let version_text = TextWidget {
            content: format!(" v{}", self.version),
            dim: true,
            ..TextWidget::default()
        };
        let version_node = version_text.build(tree);
        tree.append_child(title_row, version_node);
        tree.append_child(outer, title_row);

        // Blank line
        let blank = tree.create_element(ElementType::Text);
        let blank_text = tree.create_text_node("");
        tree.append_child(blank, blank_text);
        tree.set_style(
            blank,
            LayoutStyle {
                height: Dimension::Points(1.0),
                ..LayoutStyle::default()
            },
        );
        tree.append_child(outer, blank);

        // Model line
        let model_row = build_info_row(tree, "Model:   ", &self.model, true);
        tree.append_child(outer, model_row);

        // Tools line
        let tools_row = build_info_row(
            tree,
            "Tools:   ",
            &format!("{} registered", self.tool_count),
            false,
        );
        tree.append_child(outer, tools_row);

        // Commands line
        let cmds_row = build_info_row(
            tree,
            "Commands:",
            &format!("{} available", self.command_count),
            false,
        );
        tree.append_child(outer, cmds_row);

        // Blank line
        let blank2 = tree.create_element(ElementType::Text);
        let blank2_text = tree.create_text_node("");
        tree.append_child(blank2, blank2_text);
        tree.set_style(
            blank2,
            LayoutStyle {
                height: Dimension::Points(1.0),
                ..LayoutStyle::default()
            },
        );
        tree.append_child(outer, blank2);

        // Help hint
        let hint = TextWidget {
            content:
                "Type a message to chat, /help for commands, Ctrl+C to interrupt, Ctrl+D to exit."
                    .into(),
            dim: true,
            ..TextWidget::default()
        };
        let hint_node = hint.build(tree);
        tree.append_child(outer, hint_node);

        outer
    }
}

/// Build a label: value row.
fn build_info_row(
    tree: &mut DomTree,
    label: &str,
    value: &str,
    bold_value: bool,
) -> NodeId {
    let row = tree.create_element(ElementType::Box);
    tree.set_style(
        row,
        LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            ..LayoutStyle::default()
        },
    );

    let label_tw = TextWidget::dimmed(label);
    let label_node = label_tw.build(tree);
    tree.append_child(row, label_node);

    let value_tw = if bold_value {
        TextWidget::bold(value)
    } else {
        TextWidget::plain(value)
    };
    let value_node = value_tw.build(tree);
    tree.append_child(row, value_node);

    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_banner_build() {
        let mut tree = DomTree::new();
        let banner = WelcomeBanner {
            model: "gpt-4o".into(),
            tool_count: 12,
            command_count: 8,
            version: "0.1.0".into(),
        };
        let node = banner.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert!(elem.children.len() >= 6); // title, blank, model, tools, cmds, blank, hint
    }
}
