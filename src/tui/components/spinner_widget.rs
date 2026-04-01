//! Spinner widget -- animated loading indicator with verb and elapsed time.
//!
//! Port of `ref/components/Spinner.tsx`. Produces a DOM subtree with the
//! spinner glyph, verb text, and elapsed time counter.

use std::time::Instant;

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::Color;

use super::text_widget::TextWidget;
use super::Widget;

/// Spinner animation frames -- matches ref/components/Spinner.tsx
/// DEFAULT_CHARACTERS: braille dot pattern.
const SPINNER_CHARS_FORWARD: &[&str] = &[
    "\u{280B}", // ⠋
    "\u{2819}", // ⠙
    "\u{2839}", // ⠹
    "\u{2838}", // ⠸
    "\u{283C}", // ⠼
    "\u{2834}", // ⠴
    "\u{2826}", // ⠦
    "\u{2827}", // ⠧
    "\u{2807}", // ⠇
    "\u{280F}", // ⠏
];

/// Default spinner verbs.
const SPINNER_VERBS: &[&str] = &[
    "Thinking", "Reasoning", "Analyzing", "Considering",
    "Processing", "Evaluating", "Pondering", "Working",
];

/// Spinner widget state.
pub struct SpinnerWidget {
    /// Current animation frame index.
    pub frame: usize,
    /// When this spinner started.
    pub start_time: Instant,
    /// The verb to display (e.g., "Thinking").
    pub verb: String,
    /// Optional override message.
    pub override_message: Option<String>,
    /// Color for the spinner glyph and verb text.
    /// Default: Primary orange rgb(215,119,87) matching ref dark theme's `primary` key.
    pub color: Color,
    /// Merged forward+reversed frame list (built once).
    frames: Vec<&'static str>,
}

impl SpinnerWidget {
    /// Build the forward+reversed frame list (matching ref SpinnerGlyph.tsx).
    fn build_frames() -> Vec<&'static str> {
        let mut v: Vec<&'static str> = SPINNER_CHARS_FORWARD.to_vec();
        let mut rev: Vec<&'static str> = SPINNER_CHARS_FORWARD.to_vec();
        rev.reverse();
        v.extend(rev);
        v
    }

    /// Create a new spinner with a random verb and Primary-orange color.
    pub fn new() -> Self {
        let verb_idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize)
            % SPINNER_VERBS.len();
        Self {
            frame: 0,
            start_time: Instant::now(),
            verb: SPINNER_VERBS[verb_idx].to_string(),
            override_message: None,
            // Primary orange -- matches ref dark theme `primary: 'rgb(215,119,87)'`
            color: Color::Rgb(215, 119, 87),
            frames: Self::build_frames(),
        }
    }

    /// Advance frame counter.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Get the current spinner character.
    pub fn current_char(&self) -> &str {
        self.frames[self.frame % self.frames.len()]
    }

    /// Get the display message (verb + ellipsis).
    pub fn message(&self) -> String {
        let msg = self.override_message.as_deref().unwrap_or(&self.verb);
        format!("{}\u{2026}", msg)
    }

    /// Get elapsed time as a formatted string.
    pub fn elapsed_str(&self) -> String {
        let secs = self.start_time.elapsed().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else {
            format!("{}m{}s", secs / 60, secs % 60)
        }
    }

    /// Reset the spinner timer.
    pub fn reset_timer(&mut self) {
        self.start_time = Instant::now();
    }

    /// Set the verb text.
    pub fn set_verb(&mut self, verb: impl Into<String>) {
        self.verb = verb.into();
    }

    /// Set an override message.
    pub fn set_override(&mut self, msg: Option<String>) {
        self.override_message = msg;
    }
}

impl Clone for SpinnerWidget {
    fn clone(&self) -> Self {
        Self {
            frame: self.frame,
            start_time: self.start_time,
            verb: self.verb.clone(),
            override_message: self.override_message.clone(),
            color: self.color.clone(),
            frames: self.frames.clone(),
        }
    }
}

impl Default for SpinnerWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SpinnerWidget {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Row container: [spinner_char] [message...] [(elapsed)]
        let row = tree.create_element(ElementType::Box);
        tree.set_style(
            row,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Row,
                flex_wrap: taffy::FlexWrap::Wrap,
                padding: Edges {
                    top: 1.0,
                    bottom: 0.0,
                    left: 0.0,
                    right: 0.0,
                },
                width: Dimension::Percent(100.0),
                ..LayoutStyle::default()
            },
        );

        // Spinner char
        let spinner_text = TextWidget {
            content: self.current_char().to_string(),
            color: Some(self.color.clone()),
            ..TextWidget::default()
        };
        let spinner_node = spinner_text.build(tree);
        tree.append_child(row, spinner_node);

        // Space
        let space = tree.create_element(ElementType::Text);
        let space_text = tree.create_text_node(" ");
        tree.append_child(space, space_text);
        tree.append_child(row, space);

        // Message text
        let msg_text = TextWidget {
            content: self.message(),
            color: Some(self.color.clone()),
            ..TextWidget::default()
        };
        let msg_node = msg_text.build(tree);
        tree.append_child(row, msg_node);

        // Space
        let space2 = tree.create_element(ElementType::Text);
        let space2_text = tree.create_text_node(" ");
        tree.append_child(space2, space2_text);
        tree.append_child(row, space2);

        // Elapsed time in dim
        let elapsed_text = TextWidget {
            content: format!("({})", self.elapsed_str()),
            dim: true,
            ..TextWidget::default()
        };
        let elapsed_node = elapsed_text.build(tree);
        tree.append_child(row, elapsed_node);

        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_widget_new() {
        let s = SpinnerWidget::new();
        assert!(!s.verb.is_empty());
        assert_eq!(s.frame, 0);
    }

    #[test]
    fn test_spinner_tick() {
        let mut s = SpinnerWidget::new();
        s.tick();
        assert_eq!(s.frame, 1);
        let ch = s.current_char();
        assert!(!ch.is_empty());
    }

    #[test]
    fn test_spinner_message() {
        let s = SpinnerWidget {
            verb: "Reading".into(),
            override_message: None,
            ..SpinnerWidget::new()
        };
        assert_eq!(s.message(), "Reading\u{2026}");
    }

    #[test]
    fn test_spinner_override() {
        let s = SpinnerWidget {
            override_message: Some("Custom".into()),
            ..SpinnerWidget::new()
        };
        assert_eq!(s.message(), "Custom\u{2026}");
    }

    #[test]
    fn test_spinner_build() {
        let mut tree = DomTree::new();
        let s = SpinnerWidget::new();
        let node = s.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // spinner + space + msg + space + elapsed = 5 children
        assert_eq!(elem.children.len(), 5);
    }
}
