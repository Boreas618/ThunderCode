//! ThunderCode vim mode - a pure state machine for vim-style editing.
//!
//! This crate implements a complete vim normal-mode state machine, including:
//! - Mode switching (Insert/Normal)
//! - Motions (h, l, j, k, w, b, e, $, ^, 0, G, gg, f/F/t/T)
//! - Operators (d, c, y) with motions, text objects, and find
//! - Text objects (iw, aw, i", a(, etc.)
//! - Count prefixes (e.g. 3dw, d2w, 2d3w)
//! - Single-key commands (x, r, ~, J, p/P, o/O, D, C, Y)
//! - Indent (>>, <<)
//! - Dot repeat (.)
//! - Find repeat (; and ,)
//! - Undo (u)
//!
//! The crate has zero external dependencies beyond serde for serialization.

pub mod motions;
pub mod operators;
pub mod text_objects;
pub mod transitions;
pub mod types;

pub use operators::EditState;
pub use types::*;

/// A self-contained vim editor that wraps the state machine with a text buffer.
/// Useful for testing and simple embedding.
#[derive(Debug, Clone)]
pub struct VimEditor {
    pub text: String,
    pub cursor: usize,
    pub state: VimState,
    pub persistent: PersistentState,
    /// Set to true when enter_insert is called (so the caller can detect mode switch).
    pub entered_insert: bool,
}

impl VimEditor {
    /// Create a new VimEditor starting in normal mode with the given text.
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            cursor: 0,
            state: VimState::Normal {
                command: CommandState::Idle,
            },
            persistent: PersistentState::default(),
            entered_insert: false,
        }
    }

    /// Create a new VimEditor starting in insert mode.
    pub fn new_insert(text: &str) -> Self {
        Self {
            text: text.to_string(),
            cursor: 0,
            state: VimState::Insert {
                inserted_text: String::new(),
            },
            persistent: PersistentState::default(),
            entered_insert: false,
        }
    }

    /// Process a single keystroke in normal mode.
    /// Returns the new command state.
    pub fn feed_normal(&mut self, input: char) -> CommandState {
        self.entered_insert = false;

        let command = match &self.state {
            VimState::Normal { command } => command.clone(),
            VimState::Insert { .. } => {
                self.state = VimState::Normal {
                    command: CommandState::Idle,
                };
                CommandState::Idle
            }
        };

        let mut st = EditState {
            text: self.text.clone(),
            cursor: self.cursor,
            entered_insert: false,
            register: self.persistent.register.clone(),
            register_is_linewise: self.persistent.register_is_linewise,
            last_find: self.persistent.last_find.clone(),
            last_change: self.persistent.last_change.clone(),
        };

        let new_command = transitions::transition(&command, input, &mut st, None, None);

        // Apply state changes back
        self.text = st.text;
        self.cursor = st.cursor;
        self.persistent.register = st.register;
        self.persistent.register_is_linewise = st.register_is_linewise;
        self.persistent.last_find = st.last_find;
        self.persistent.last_change = st.last_change;
        self.entered_insert = st.entered_insert;

        if st.entered_insert {
            self.state = VimState::Insert {
                inserted_text: String::new(),
            };
        } else {
            self.state = VimState::Normal {
                command: new_command.clone(),
            };
        }

        new_command
    }

    /// Feed a sequence of normal-mode keystrokes.
    pub fn feed_normal_str(&mut self, keys: &str) {
        for ch in keys.chars() {
            self.feed_normal(ch);
        }
    }

    /// Switch to normal mode (like pressing Escape).
    pub fn escape(&mut self) {
        self.state = VimState::Normal {
            command: CommandState::Idle,
        };
        // In vim, Escape in normal mode moves cursor back one if not at line start
        if self.cursor > 0 {
            let line_start = motions::start_of_logical_line(&self.text, self.cursor);
            if self.cursor > line_start {
                let new_pos = self.text[..self.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                self.cursor = new_pos.max(line_start);
            }
        }
    }

    /// Insert text at current cursor position (simulating insert mode typing).
    pub fn insert_text(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        if let VimState::Insert { inserted_text } = &mut self.state {
            inserted_text.push_str(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Test 1: Mode switching (i enters insert, Escape returns to normal)
    // ========================================================================
    #[test]
    fn test_mode_switching_insert_and_back() {
        let mut ed = VimEditor::new("hello");
        assert!(ed.state.is_normal());

        ed.feed_normal('i');
        assert!(ed.entered_insert);
        assert!(ed.state.is_insert());

        ed.escape();
        assert!(ed.state.is_normal());
    }

    // ========================================================================
    // Test 2: Basic motions (h, l, w, b, 0, $, ^)
    // ========================================================================
    #[test]
    fn test_basic_motions() {
        let mut ed = VimEditor::new("hello world");

        ed.feed_normal('l');
        assert_eq!(ed.cursor, 1);

        ed.feed_normal('h');
        assert_eq!(ed.cursor, 0);

        ed.feed_normal('h');
        assert_eq!(ed.cursor, 0);

        ed.feed_normal('w');
        assert_eq!(ed.cursor, 6);

        ed.feed_normal('b');
        assert_eq!(ed.cursor, 0);

        ed.feed_normal('$');
        assert_eq!(ed.cursor, 10); // last char 'd'

        ed.feed_normal('0');
        assert_eq!(ed.cursor, 0);
    }

    // ========================================================================
    // Test 3: Count prefix (e.g. 3l, 2w)
    // ========================================================================
    #[test]
    fn test_count_prefix() {
        let mut ed = VimEditor::new("hello world foo bar");

        ed.feed_normal_str("3l");
        assert_eq!(ed.cursor, 3);

        ed.cursor = 0;

        ed.feed_normal_str("2w");
        assert_eq!(ed.cursor, 12); // "foo"
    }

    // ========================================================================
    // Test 4: Delete operator + motion (dw, d$)
    // ========================================================================
    #[test]
    fn test_delete_operator_with_motion() {
        let mut ed = VimEditor::new("hello world");
        ed.feed_normal_str("dw");
        assert_eq!(ed.text, "world");
        assert_eq!(ed.cursor, 0);

        let mut ed = VimEditor::new("hello world");
        ed.cursor = 5;
        ed.feed_normal_str("d$");
        assert_eq!(ed.text, "hello");
    }

    // ========================================================================
    // Test 5: dd (delete line)
    // ========================================================================
    #[test]
    fn test_dd_delete_line() {
        let mut ed = VimEditor::new("line1\nline2\nline3");
        ed.feed_normal_str("dd");
        assert_eq!(ed.text, "line2\nline3");
        assert_eq!(ed.cursor, 0);
        assert!(ed.persistent.register.ends_with('\n'));
        assert!(ed.persistent.register.contains("line1"));
    }

    // ========================================================================
    // Test 6: yy (yank line) and p (paste)
    // ========================================================================
    #[test]
    fn test_yy_and_paste() {
        let mut ed = VimEditor::new("line1\nline2");
        ed.feed_normal_str("yy");
        assert_eq!(ed.text, "line1\nline2");
        assert!(ed.persistent.register.contains("line1"));

        ed.feed_normal('p');
        assert_eq!(ed.text, "line1\nline1\nline2");
    }

    // ========================================================================
    // Test 7: Text objects (diw, di")
    // ========================================================================
    #[test]
    fn test_text_objects() {
        let mut ed = VimEditor::new("hello world");
        ed.cursor = 1;
        ed.feed_normal_str("diw");
        assert_eq!(ed.text, " world");

        let mut ed = VimEditor::new(r#"say "hello" ok"#);
        ed.cursor = 6;
        ed.feed_normal_str("di\"");
        assert_eq!(ed.text, r#"say "" ok"#);
    }

    // ========================================================================
    // Test 8: Change operator (cw enters insert)
    // ========================================================================
    #[test]
    fn test_change_word() {
        let mut ed = VimEditor::new("hello world");
        ed.feed_normal_str("cw");
        assert!(ed.state.is_insert());
        assert!(ed.entered_insert);
        assert_eq!(ed.text, " world");
    }

    // ========================================================================
    // Test 9: x (delete char) and r (replace char)
    // ========================================================================
    #[test]
    fn test_x_and_r() {
        let mut ed = VimEditor::new("hello");
        ed.feed_normal('x');
        assert_eq!(ed.text, "ello");

        let mut ed = VimEditor::new("hello");
        ed.feed_normal_str("ra");
        assert_eq!(ed.text, "aello");
        assert_eq!(ed.cursor, 0);
    }

    // ========================================================================
    // Test 10: Find motions (f, F, t, T) and repeat (; ,)
    // ========================================================================
    #[test]
    fn test_find_motions() {
        let mut ed = VimEditor::new("hello world");

        ed.feed_normal_str("fo");
        assert_eq!(ed.cursor, 4);

        ed.feed_normal(';');
        assert_eq!(ed.cursor, 7);

        ed.feed_normal(',');
        assert_eq!(ed.cursor, 4);
    }

    // ========================================================================
    // Test 11: j/k (up/down movement)
    // ========================================================================
    #[test]
    fn test_j_k_movement() {
        let mut ed = VimEditor::new("abc\ndef\nghi");

        ed.feed_normal('j');
        assert_eq!(ed.cursor, 4);

        ed.feed_normal('j');
        assert_eq!(ed.cursor, 8);

        ed.feed_normal('k');
        assert_eq!(ed.cursor, 4);

        ed.feed_normal('k');
        assert_eq!(ed.cursor, 0);
    }

    // ========================================================================
    // Test 12: G and gg motions
    // ========================================================================
    #[test]
    fn test_g_and_gg() {
        let mut ed = VimEditor::new("line1\nline2\nline3\nline4");

        ed.feed_normal('G');
        assert_eq!(ed.cursor, motions::start_of_last_line(&ed.text));

        ed.feed_normal_str("gg");
        assert_eq!(ed.cursor, 0);

        ed.feed_normal_str("3G");
        let expected = motions::go_to_line(&ed.text, 3);
        assert_eq!(ed.cursor, expected);
    }

    // ========================================================================
    // Test 13: Indent (>> and <<)
    // ========================================================================
    #[test]
    fn test_indent() {
        let mut ed = VimEditor::new("hello\nworld");

        ed.feed_normal_str(">>");
        assert_eq!(ed.text, "  hello\nworld");

        ed.feed_normal_str("<<");
        assert_eq!(ed.text, "hello\nworld");
    }

    // ========================================================================
    // Test 14: o/O (open line above/below)
    // ========================================================================
    #[test]
    fn test_open_line() {
        let mut ed = VimEditor::new("line1\nline2");
        ed.feed_normal('o');
        assert!(ed.state.is_insert());
        assert_eq!(ed.text, "line1\n\nline2");

        let mut ed = VimEditor::new("line1\nline2");
        ed.feed_normal('O');
        assert!(ed.state.is_insert());
        assert_eq!(ed.text, "\nline1\nline2");
    }

    // ========================================================================
    // Test 15: Operator + count (d2w, 2dw)
    // ========================================================================
    #[test]
    fn test_operator_with_count() {
        let mut ed = VimEditor::new("one two three four");
        ed.feed_normal_str("d2w");
        assert_eq!(ed.text, "three four");

        let mut ed = VimEditor::new("one two three four");
        ed.feed_normal_str("2dw");
        assert_eq!(ed.text, "three four");
    }

    // ========================================================================
    // Test 16: cc (change line)
    // ========================================================================
    #[test]
    fn test_cc_change_line() {
        let mut ed = VimEditor::new("hello\nworld");
        ed.feed_normal_str("cc");
        assert!(ed.state.is_insert());
        assert_eq!(ed.text, "\nworld");
    }

    // ========================================================================
    // Test 17: D and C shortcuts
    // ========================================================================
    #[test]
    fn test_d_and_c_shortcuts() {
        let mut ed = VimEditor::new("hello world");
        ed.cursor = 5;
        ed.feed_normal('D');
        assert_eq!(ed.text, "hello");

        let mut ed = VimEditor::new("hello world");
        ed.cursor = 5;
        ed.feed_normal('C');
        assert_eq!(ed.text, "hello");
        assert!(ed.state.is_insert());
    }

    // ========================================================================
    // Test 18: ~ (toggle case)
    // ========================================================================
    #[test]
    fn test_toggle_case() {
        let mut ed = VimEditor::new("Hello");
        ed.feed_normal('~');
        assert_eq!(ed.text, "hello");

        ed.feed_normal('~');
        assert_eq!(ed.text, "hEllo");
    }

    // ========================================================================
    // Test 19: J (join lines)
    // ========================================================================
    #[test]
    fn test_join_lines() {
        let mut ed = VimEditor::new("hello\nworld");
        ed.feed_normal('J');
        assert_eq!(ed.text, "hello world");
    }

    // ========================================================================
    // Test 20: a and A (append)
    // ========================================================================
    #[test]
    fn test_append() {
        let mut ed = VimEditor::new("hello");
        ed.feed_normal('a');
        assert!(ed.state.is_insert());
        assert_eq!(ed.cursor, 1);

        let mut ed = VimEditor::new("hello");
        ed.feed_normal('A');
        assert!(ed.state.is_insert());
        assert_eq!(ed.cursor, 5);
    }

    // ========================================================================
    // Test 21: I (insert at first non-blank)
    // ========================================================================
    #[test]
    fn test_insert_at_first_non_blank() {
        let mut ed = VimEditor::new("  hello");
        ed.cursor = 4;
        ed.feed_normal('I');
        assert!(ed.state.is_insert());
        assert_eq!(ed.cursor, 2);
    }

    // ========================================================================
    // Test 22: daw (delete a word)
    // ========================================================================
    #[test]
    fn test_daw() {
        let mut ed = VimEditor::new("hello world foo");
        ed.cursor = 7;
        ed.feed_normal_str("daw");
        assert_eq!(ed.text, "hello foo");
    }

    // ========================================================================
    // Test 23: Bracket text objects (di()
    // ========================================================================
    #[test]
    fn test_bracket_text_objects() {
        let mut ed = VimEditor::new("foo(bar)baz");
        ed.cursor = 5;
        ed.feed_normal_str("di(");
        assert_eq!(ed.text, "foo()baz");
    }

    // ========================================================================
    // Test 24: e motion (end of word)
    // ========================================================================
    #[test]
    fn test_e_motion() {
        let mut ed = VimEditor::new("hello world");
        ed.feed_normal('e');
        assert_eq!(ed.cursor, 4);
    }

    // ========================================================================
    // Test 25: 2dd (delete 2 lines)
    // ========================================================================
    #[test]
    fn test_count_dd() {
        let mut ed = VimEditor::new("line1\nline2\nline3\nline4");
        ed.feed_normal_str("2dd");
        assert_eq!(ed.text, "line3\nline4");
    }

    // ========================================================================
    // Test 26: ^ motion (first non-blank)
    // ========================================================================
    #[test]
    fn test_caret_motion() {
        let mut ed = VimEditor::new("   hello");
        ed.cursor = 6;
        ed.feed_normal('^');
        assert_eq!(ed.cursor, 3);
    }

    // ========================================================================
    // Test 27: t motion (till)
    // ========================================================================
    #[test]
    fn test_till_motions() {
        let mut ed = VimEditor::new("hello world");
        ed.feed_normal_str("to");
        assert_eq!(ed.cursor, 3);
    }

    // ========================================================================
    // Test 28: Y (yank line)
    // ========================================================================
    #[test]
    fn test_yank_line() {
        let mut ed = VimEditor::new("line1\nline2");
        ed.feed_normal('Y');
        assert!(ed.persistent.register.contains("line1"));
        assert_eq!(ed.text, "line1\nline2");
    }

    // ========================================================================
    // Test 29: Operator + find (dfo)
    // ========================================================================
    #[test]
    fn test_operator_find() {
        let mut ed = VimEditor::new("hello world");
        ed.feed_normal_str("dfo");
        assert_eq!(ed.text, " world");
    }

    // ========================================================================
    // Test 30: Cancel incomplete commands
    // ========================================================================
    #[test]
    fn test_cancel_incomplete() {
        let mut ed = VimEditor::new("hello");
        ed.feed_normal('d');
        ed.feed_normal('z'); // invalid -> reset
        assert_eq!(
            ed.state,
            VimState::Normal {
                command: CommandState::Idle
            }
        );
        assert_eq!(ed.text, "hello");
    }
}
