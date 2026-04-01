//! Prompt input field -- the main chat input widget.
//!
//! Handles multi-line editing, cursor positioning, and history navigation.
//! Port of `ref/components/PromptInput/PromptInput.tsx`.

use crate::tui::dom::{DomTree, NodeId};
use crate::tui::layout::{
    Dimension, Edges, ElementType, LayoutFlexDirection, LayoutStyle,
};
use crate::tui::style::Color;
use crate::tui::text::string_width;

use super::text_widget::TextWidget;
use super::Widget;

/// Chat prompt input state and editing buffer.
pub struct PromptInput {
    /// Current input buffer text.
    buffer: String,
    /// Cursor position (byte offset into buffer).
    cursor: usize,
    /// Whether this prompt is currently focused/active.
    active: bool,
    /// Input history for up/down navigation.
    history: Vec<String>,
    /// Current history navigation index (None = editing current input).
    history_index: Option<usize>,
    /// Saved current input when navigating history.
    saved_input: String,
}

impl PromptInput {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            active: true,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
        }
    }

    /// Get the current buffer content.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Get the cursor position (byte offset).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the input is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Set active state.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Insert a string at the cursor position (for paste).
    pub fn insert_str(&mut self, s: &str) {
        self.buffer.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_backward(&mut self) {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.buffer.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete_forward(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.buffer[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.buffer.len());
            self.buffer.drain(self.cursor..next);
        }
    }

    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor = self.buffer[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.buffer.len());
        }
    }

    /// Move cursor to the beginning of the line.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Kill from cursor to beginning of line (Ctrl+U).
    pub fn kill_to_start(&mut self) {
        self.buffer.drain(..self.cursor);
        self.cursor = 0;
    }

    /// Kill word backward (Ctrl+W).
    pub fn kill_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let old_cursor = self.cursor;
        while self.cursor > 0 && self.buffer.as_bytes().get(self.cursor - 1) == Some(&b' ') {
            self.cursor -= 1;
        }
        while self.cursor > 0 && self.buffer.as_bytes().get(self.cursor - 1) != Some(&b' ') {
            self.cursor -= 1;
        }
        self.buffer.drain(self.cursor..old_cursor);
    }

    /// Navigate up in history.
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.saved_input = self.buffer.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(idx) if idx > 0 => {
                self.history_index = Some(idx - 1);
            }
            _ => return,
        }
        if let Some(idx) = self.history_index {
            self.buffer = self.history[idx].clone();
            self.cursor = self.buffer.len();
        }
    }

    /// Navigate down in history.
    pub fn history_down(&mut self) {
        match self.history_index {
            Some(idx) => {
                if idx + 1 < self.history.len() {
                    self.history_index = Some(idx + 1);
                    self.buffer = self.history[idx + 1].clone();
                } else {
                    self.history_index = None;
                    self.buffer = self.saved_input.clone();
                }
                self.cursor = self.buffer.len();
            }
            None => {}
        }
    }

    /// Submit the current input, add to history, and reset.
    /// Returns the submitted text.
    pub fn submit(&mut self) -> String {
        let text = self.buffer.clone();
        if !text.trim().is_empty() {
            self.history.push(text.clone());
        }
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_input.clear();
        text
    }

    /// Clear the current input without submitting.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
    }

    /// Get the display width of the buffer text before the cursor.
    pub fn cursor_display_col(&self) -> usize {
        string_width(&self.buffer[..self.cursor])
    }

    /// Render the prompt input as an ANSI string for direct terminal output.
    /// Returns (prompt_line, cursor_col) where cursor_col is the 0-based
    /// display column for the terminal cursor (after the "❯ " prefix).
    ///
    /// Ref: PromptInputModeIndicator renders `❯ ` (figures.pointer + space).
    /// In normal mode, the pointer has no special color (inherits text).
    /// When loading, it is dimmed.
    pub fn render(&self) -> (String, usize) {
        // The ref's PromptChar renders: <Text>{figures.pointer} </Text>
        // figures.pointer = ❯ (U+276F) on macOS.
        // No explicit color = inherits default text color.
        let prefix = "\u{276F} ";
        let prefix_width = 2; // "❯ " is 2 display columns
        let cursor_col = prefix_width + self.cursor_display_col();
        let line = format!("{}{}", prefix, self.buffer);
        (line, cursor_col)
    }
}

impl Default for PromptInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for PromptInput {
    fn build(&self, tree: &mut DomTree) -> NodeId {
        // Ref: PromptInput uses a rounded border box:
        //   <Box borderStyle="round" borderLeft={false} borderRight={false}
        //        borderBottom borderColor={promptBorder}>
        //     <PromptInputModeIndicator />  (renders "❯ ")
        //     <TextInput />
        //   </Box>
        //
        // borderStyle="round" with only top+bottom borders renders:
        //   ─────────────  (top border: ─ repeated, using promptBorder color)
        //   ❯ user input
        //   ─────────────  (bottom border)
        //
        // promptBorder = rgb(136,136,136) on dark theme.

        // Outer column: [top border] [input row] [bottom border]
        let outer = tree.create_element(ElementType::Box);
        let outer_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Column,
            width: Dimension::Percent(100.0),
            margin: Edges {
                top: 1.0,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        };
        tree.set_style(outer, outer_style);

        // Top border: ─ in promptBorder color
        let top_border = TextWidget {
            content: "\u{2500}".repeat(80), // will be clipped to width by renderer
            color: Some(Color::Rgb(136, 136, 136)),
            ..TextWidget::default()
        };
        let top_node = top_border.build(tree);
        tree.append_child(outer, top_node);

        // Input row
        let input_row = tree.create_element(ElementType::Box);
        let input_style = LayoutStyle {
            flex_direction: LayoutFlexDirection::Row,
            width: Dimension::Percent(100.0),
            min_height: Dimension::Points(1.0),
            ..LayoutStyle::default()
        };
        tree.set_style(input_row, input_style);

        // Prompt prefix: "❯ " (figures.pointer + space).
        // Ref: PromptChar renders <Text>{figures.pointer} </Text>
        // No explicit color in normal mode (inherits text color).
        let prefix = TextWidget {
            content: "\u{276F} ".into(),
            ..TextWidget::default()
        };
        let prefix_node = prefix.build(tree);
        tree.append_child(input_row, prefix_node);

        if self.active {
            // Text before cursor
            let before_cursor = &self.buffer[..self.cursor];
            if !before_cursor.is_empty() {
                let before_tw = TextWidget::plain(before_cursor);
                let before_node = before_tw.build(tree);
                tree.append_child(input_row, before_node);
            }

            // Cursor character (block cursor via inverse)
            let cursor_char = if self.cursor < self.buffer.len() {
                let next_end = self.buffer[self.cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.cursor + i)
                    .unwrap_or(self.buffer.len());
                self.buffer[self.cursor..next_end].to_string()
            } else {
                " ".to_string()
            };
            let cursor_tw = TextWidget {
                content: cursor_char,
                inverse: true,
                ..TextWidget::default()
            };
            let cursor_node = cursor_tw.build(tree);
            tree.append_child(input_row, cursor_node);

            // Text after cursor
            if self.cursor < self.buffer.len() {
                let after_start = self.buffer[self.cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.cursor + i)
                    .unwrap_or(self.buffer.len());
                let after_cursor = &self.buffer[after_start..];
                if !after_cursor.is_empty() {
                    let after_tw = TextWidget::plain(after_cursor);
                    let after_node = after_tw.build(tree);
                    tree.append_child(input_row, after_node);
                }
            }
        } else {
            // Inactive: just show the text dimmed, no cursor
            if !self.buffer.is_empty() {
                let tw = TextWidget::dimmed(&self.buffer);
                let text_node = tw.build(tree);
                tree.append_child(input_row, text_node);
            }
        }

        tree.append_child(outer, input_row);

        // Bottom border: ─ in promptBorder color
        let bottom_border = TextWidget {
            content: "\u{2500}".repeat(80),
            color: Some(Color::Rgb(136, 136, 136)),
            ..TextWidget::default()
        };
        let bottom_node = bottom_border.build(tree);
        tree.append_child(outer, bottom_node);

        outer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_input_new() {
        let p = PromptInput::new();
        assert!(p.buffer().is_empty());
        assert_eq!(p.cursor(), 0);
        assert!(p.is_active());
    }

    #[test]
    fn test_insert_and_delete() {
        let mut p = PromptInput::new();
        p.insert_char('h');
        p.insert_char('i');
        assert_eq!(p.buffer(), "hi");
        assert_eq!(p.cursor(), 2);

        p.delete_backward();
        assert_eq!(p.buffer(), "h");
        assert_eq!(p.cursor(), 1);
    }

    #[test]
    fn test_cursor_movement() {
        let mut p = PromptInput::new();
        p.insert_str("abc");
        p.move_left();
        assert_eq!(p.cursor(), 2);
        p.move_home();
        assert_eq!(p.cursor(), 0);
        p.move_end();
        assert_eq!(p.cursor(), 3);
    }

    #[test]
    fn test_history() {
        let mut p = PromptInput::new();
        p.insert_str("first");
        p.submit();
        p.insert_str("second");
        p.submit();

        p.history_up();
        assert_eq!(p.buffer(), "second");
        p.history_up();
        assert_eq!(p.buffer(), "first");
        p.history_down();
        assert_eq!(p.buffer(), "second");
    }

    #[test]
    fn test_kill_word_backward() {
        let mut p = PromptInput::new();
        p.insert_str("hello world");
        p.kill_word_backward();
        assert_eq!(p.buffer(), "hello ");
    }

    #[test]
    fn test_prompt_input_build_active() {
        let mut tree = DomTree::new();
        let mut p = PromptInput::new();
        p.insert_str("test");
        let node = p.build(&mut tree);
        let elem = tree.element(node).unwrap();
        assert_eq!(elem.element_type, ElementType::Box);
        // Outer column: top_border + input_row + bottom_border = 3 children
        assert_eq!(elem.children.len(), 3);
    }

    #[test]
    fn test_prompt_input_build_inactive() {
        let mut tree = DomTree::new();
        let mut p = PromptInput::new();
        p.insert_str("text");
        p.set_active(false);
        let node = p.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // Outer column: top_border + input_row + bottom_border = 3 children
        assert_eq!(elem.children.len(), 3);
    }

    #[test]
    fn test_prompt_input_build_empty_active() {
        let mut tree = DomTree::new();
        let p = PromptInput::new();
        let node = p.build(&mut tree);
        let elem = tree.element(node).unwrap();
        // Outer column: top_border + input_row + bottom_border = 3 children
        assert_eq!(elem.children.len(), 3);
    }
}
