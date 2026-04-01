//! Vim operator functions.
//!
//! Functions for executing vim operators (delete, change, yank, etc.)

use crate::vim::motions::{
    self, find_character, is_inclusive_motion, is_linewise_motion, line_of_offset, next_offset,
    resolve_motion, start_of_logical_line,
};
use crate::vim::text_objects::find_text_object;
use crate::vim::types::{
    FindType, LastFind, OpenLineDirection, Operator, RecordedChange, TextObjScope,
};

/// Mutable editing state passed through all operator / transition functions.
///
/// Instead of closures (which fight the borrow checker) every operation
/// mutates this struct directly.  The owner copies values back out afterwards.
pub struct EditState {
    pub text: String,
    pub cursor: usize,
    pub entered_insert: bool,
    pub register: String,
    pub register_is_linewise: bool,
    pub last_find: Option<LastFind>,
    pub last_change: Option<RecordedChange>,
}

impl EditState {
    /// Convenience: set cursor.
    pub fn set_offset(&mut self, offset: usize) {
        self.cursor = offset;
    }

    /// Convenience: enter insert mode at offset.
    pub fn enter_insert(&mut self, offset: usize) {
        self.cursor = offset;
        self.entered_insert = true;
    }

    /// Convenience: set register.
    pub fn set_register(&mut self, content: String, linewise: bool) {
        self.register = content;
        self.register_is_linewise = linewise;
    }

    /// Convenience: set last find.
    pub fn set_last_find(&mut self, ft: FindType, ch: char) {
        self.last_find = Some(LastFind {
            find_type: ft,
            ch,
        });
    }

    /// Convenience: record a change.
    pub fn record_change(&mut self, change: RecordedChange) {
        self.last_change = Some(change);
    }
}

// ============================================================================
// Operator + Motion
// ============================================================================

/// Execute an operator with a simple motion.
pub fn execute_operator_motion(op: Operator, motion: &str, count: usize, st: &mut EditState) {
    let target = resolve_motion(&st.text, st.cursor, motion, count);
    if target == st.cursor {
        return;
    }

    let range = get_operator_range(&st.text, st.cursor, target, motion, op, count);
    apply_operator(op, range.from, range.to, st, range.linewise);
    st.record_change(RecordedChange::Operator {
        op,
        motion: motion.to_string(),
        count,
    });
}

/// Execute an operator with a find motion.
pub fn execute_operator_find(
    op: Operator,
    find_type: FindType,
    ch: char,
    count: usize,
    st: &mut EditState,
) {
    let target_offset = find_character(&st.text, st.cursor, ch, find_type, count);
    let target_offset = match target_offset {
        Some(o) => o,
        None => return,
    };

    let range = get_operator_range_for_find(&st.text, st.cursor, target_offset);
    apply_operator(op, range.from, range.to, st, false);
    st.set_last_find(find_type, ch);
    st.record_change(RecordedChange::OperatorFind {
        op,
        find: find_type,
        ch,
        count,
    });
}

/// Execute an operator with a text object.
pub fn execute_operator_text_obj(
    op: Operator,
    scope: TextObjScope,
    obj_type: char,
    count: usize,
    st: &mut EditState,
) {
    let range = find_text_object(&st.text, st.cursor, obj_type, scope == TextObjScope::Inner);
    let range = match range {
        Some(r) => r,
        None => return,
    };

    apply_operator(op, range.start, range.end, st, false);
    st.record_change(RecordedChange::OperatorTextObj {
        op,
        obj_type: obj_type.to_string(),
        scope,
        count,
    });
}

/// Execute a line operation (dd, cc, yy).
pub fn execute_line_op(op: Operator, count: usize, st: &mut EditState) {
    // Clone text to avoid borrow conflicts when mutating st later.
    let text = st.text.clone();
    let lines: Vec<&str> = text.split('\n').collect();
    let current_line = line_of_offset(&text, st.cursor);
    let lines_to_affect = count.min(lines.len() - current_line);
    let line_start = start_of_logical_line(&text, st.cursor);

    let mut line_end = line_start;
    for _ in 0..lines_to_affect {
        match text[line_end..].find('\n') {
            Some(pos) => line_end = line_end + pos + 1,
            None => {
                line_end = text.len();
                break;
            }
        }
    }

    let mut content = text[line_start..line_end].to_string();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    st.set_register(content, true);

    match op {
        Operator::Yank => {
            st.set_offset(line_start);
        }
        Operator::Delete => {
            let mut delete_start = line_start;
            let delete_end = line_end;

            if delete_end == text.len()
                && delete_start > 0
                && text.as_bytes()[delete_start - 1] == b'\n'
            {
                delete_start -= 1;
            }

            let new_text = format!("{}{}", &text[..delete_start], &text[delete_end..]);
            let max_off = if new_text.is_empty() {
                0
            } else {
                new_text.len().saturating_sub(last_char_len(&new_text))
            };
            st.set_offset(delete_start.min(max_off));
            st.text = new_text;
        }
        Operator::Change => {
            if lines.len() == 1 {
                st.text = String::new();
                st.enter_insert(0);
            } else {
                let before: Vec<&str> = lines[..current_line].to_vec();
                let after: Vec<&str> = lines[(current_line + lines_to_affect)..].to_vec();
                let mut new_lines = before;
                new_lines.push("");
                new_lines.extend(after);
                st.text = new_lines.join("\n");
                st.enter_insert(line_start);
            }
        }
    }

    st.record_change(RecordedChange::Operator {
        op,
        motion: op.key().to_string(),
        count,
    });
}

// ============================================================================
// Single-key commands
// ============================================================================

/// Execute delete character (x command).
pub fn execute_x(count: usize, st: &mut EditState) {
    let from = st.cursor;
    if from >= st.text.len() {
        return;
    }

    let mut end = from;
    for _ in 0..count {
        if end >= st.text.len() {
            break;
        }
        end = next_offset(&st.text, end);
    }

    let deleted = st.text[from..end].to_string();
    let new_text = format!("{}{}", &st.text[..from], &st.text[end..]);
    st.set_register(deleted, false);
    let max_off = if new_text.is_empty() {
        0
    } else {
        new_text.len().saturating_sub(last_char_len(&new_text))
    };
    st.set_offset(from.min(max_off));
    st.text = new_text;
    st.record_change(RecordedChange::X { count });
}

/// Execute replace character (r command).
pub fn execute_replace(ch: char, count: usize, st: &mut EditState) {
    let mut offset = st.cursor;
    let mut new_text = st.text.clone();

    for _ in 0..count {
        if offset >= new_text.len() {
            break;
        }
        let char_len = new_text[offset..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        let replacement = ch.to_string();
        new_text = format!(
            "{}{}{}",
            &new_text[..offset],
            replacement,
            &new_text[offset + char_len..]
        );
        offset += replacement.len();
    }

    st.text = new_text;
    st.set_offset(offset.saturating_sub(ch.len_utf8()));
    st.record_change(RecordedChange::Replace { ch, count });
}

/// Execute toggle case (~ command).
pub fn execute_toggle_case(count: usize, st: &mut EditState) {
    let start_offset = st.cursor;
    if start_offset >= st.text.len() {
        return;
    }

    let mut new_text = st.text.clone();
    let mut offset = start_offset;
    let mut toggled = 0;

    while offset < new_text.len() && toggled < count {
        let ch = new_text[offset..].chars().next().unwrap();
        let char_len = ch.len_utf8();

        let toggled_ch: String = if ch.is_uppercase() {
            ch.to_lowercase().to_string()
        } else {
            ch.to_uppercase().to_string()
        };

        new_text = format!(
            "{}{}{}",
            &new_text[..offset],
            toggled_ch,
            &new_text[offset + char_len..]
        );
        offset += toggled_ch.len();
        toggled += 1;
    }

    st.text = new_text;
    st.set_offset(offset);
    st.record_change(RecordedChange::ToggleCase { count });
}

/// Execute join lines (J command).
pub fn execute_join(count: usize, st: &mut EditState) {
    let text = &st.text;
    let lines: Vec<&str> = text.split('\n').collect();
    let current_line = line_of_offset(text, st.cursor);

    if current_line >= lines.len() - 1 {
        return;
    }

    let lines_to_join = count.min(lines.len() - current_line - 1);
    let mut joined_line = lines[current_line].to_string();
    let cursor_pos = joined_line.len();

    for i in 1..=lines_to_join {
        let next_line = lines[current_line + i].trim_start();
        if !next_line.is_empty() {
            if !joined_line.ends_with(' ') && !joined_line.is_empty() {
                joined_line.push(' ');
            }
            joined_line.push_str(next_line);
        }
    }

    let mut parts: Vec<String> = lines[..current_line].iter().map(|s| s.to_string()).collect();
    parts.push(joined_line);
    parts.extend(
        lines[(current_line + lines_to_join + 1)..]
            .iter()
            .map(|s| s.to_string()),
    );
    let new_text = parts.join("\n");

    let line_start_offset = get_line_start_offset_from_parts(&parts, current_line);
    st.text = new_text;
    st.set_offset(line_start_offset + cursor_pos);
    st.record_change(RecordedChange::Join { count });
}

/// Execute paste (p/P command).
pub fn execute_paste(after: bool, count: usize, st: &mut EditState) {
    let register = st.register.clone();
    let reg_linewise = st.register_is_linewise;
    if register.is_empty() {
        return;
    }

    let is_linewise = reg_linewise || register.ends_with('\n');
    let content = if is_linewise && register.ends_with('\n') {
        &register[..register.len() - 1]
    } else {
        &register
    };

    if is_linewise {
        let text = &st.text;
        let lines: Vec<&str> = text.split('\n').collect();
        let current_line = line_of_offset(text, st.cursor);

        let insert_line = if after { current_line + 1 } else { current_line };
        let content_lines: Vec<&str> = content.split('\n').collect();
        let mut repeated_lines: Vec<&str> = Vec::new();
        for _ in 0..count {
            repeated_lines.extend_from_slice(&content_lines);
        }

        let mut new_lines: Vec<&str> = lines[..insert_line].to_vec();
        new_lines.extend_from_slice(&repeated_lines);
        new_lines.extend_from_slice(&lines[insert_line..]);

        let new_text = new_lines.join("\n");
        let offset = get_line_start_offset_strs(&new_lines, insert_line);
        st.text = new_text;
        st.set_offset(offset);
    } else {
        let text_to_insert: String = content.repeat(count);
        let insert_point = if after && st.cursor < st.text.len() {
            next_offset(&st.text, st.cursor)
        } else {
            st.cursor
        };

        let new_text = format!(
            "{}{}{}",
            &st.text[..insert_point],
            text_to_insert,
            &st.text[insert_point..]
        );

        let last_char_size = last_char_len(&text_to_insert).max(1);
        let new_offset = insert_point + text_to_insert.len() - last_char_size;
        st.text = new_text;
        st.set_offset(insert_point.max(new_offset));
    }
}

/// Execute indent (>> or << command).
pub fn execute_indent(dir: char, count: usize, st: &mut EditState) {
    let text = &st.text;
    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    let current_line = line_of_offset(text, st.cursor);
    let lines_to_affect = count.min(lines.len() - current_line);
    let indent = "  "; // Two spaces

    for i in 0..lines_to_affect {
        let line_idx = current_line + i;
        let line = &lines[line_idx];

        if dir == '>' {
            lines[line_idx] = format!("{}{}", indent, line);
        } else if line.starts_with(indent) {
            lines[line_idx] = line[indent.len()..].to_string();
        } else if line.starts_with('\t') {
            lines[line_idx] = line[1..].to_string();
        } else {
            let mut removed = 0;
            let mut idx = 0;
            let line_bytes = line.as_bytes();
            while idx < line_bytes.len()
                && removed < indent.len()
                && (line_bytes[idx] as char).is_whitespace()
            {
                removed += 1;
                idx += 1;
            }
            lines[line_idx] = line[idx..].to_string();
        }
    }

    let new_text = lines.join("\n");
    let current_line_text = &lines[current_line];
    let first_non_blank = current_line_text
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(0);

    let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    let line_start = get_line_start_offset_strs(&line_refs, current_line);
    st.text = new_text;
    st.set_offset(line_start + first_non_blank);
    st.record_change(RecordedChange::Indent { dir, count });
}

/// Execute open line (o/O command).
pub fn execute_open_line(direction: OpenLineDirection, st: &mut EditState) {
    let text = &st.text;
    let lines: Vec<&str> = text.split('\n').collect();
    let current_line = line_of_offset(text, st.cursor);

    let insert_line = match direction {
        OpenLineDirection::Below => current_line + 1,
        OpenLineDirection::Above => current_line,
    };

    let mut new_lines: Vec<&str> = lines[..insert_line].to_vec();
    new_lines.push("");
    new_lines.extend_from_slice(&lines[insert_line..]);

    let new_text = new_lines.join("\n");
    let offset = get_line_start_offset_strs(&new_lines, insert_line);
    st.text = new_text;
    st.enter_insert(offset);
    st.record_change(RecordedChange::OpenLine { direction });
}

/// Execute operator with G motion (dG, cG, yG).
pub fn execute_operator_g_motion(op: Operator, count: usize, st: &mut EditState) {
    let target = if count == 1 {
        motions::start_of_last_line(&st.text)
    } else {
        motions::go_to_line(&st.text, count)
    };

    if target == st.cursor {
        return;
    }

    let range = get_operator_range(&st.text, st.cursor, target, "G", op, count);
    apply_operator(op, range.from, range.to, st, range.linewise);
    st.record_change(RecordedChange::Operator {
        op,
        motion: "G".to_string(),
        count,
    });
}

/// Execute operator with gg motion (dgg, cgg, ygg).
pub fn execute_operator_gg(op: Operator, count: usize, st: &mut EditState) {
    let target = if count == 1 {
        motions::start_of_first_line(&st.text)
    } else {
        motions::go_to_line(&st.text, count)
    };

    if target == st.cursor {
        return;
    }

    let range = get_operator_range(&st.text, st.cursor, target, "gg", op, count);
    apply_operator(op, range.from, range.to, st, range.linewise);
    st.record_change(RecordedChange::Operator {
        op,
        motion: "gg".to_string(),
        count,
    });
}

// ============================================================================
// Internal helpers
// ============================================================================

struct OperatorRange {
    from: usize,
    to: usize,
    linewise: bool,
}

fn get_operator_range(
    text: &str,
    cursor: usize,
    target: usize,
    motion: &str,
    op: Operator,
    count: usize,
) -> OperatorRange {
    let mut from = cursor.min(target);
    let mut to = cursor.max(target);
    let mut linewise = false;

    // Special case: cw/cW changes to end of word, not start of next word
    if op == Operator::Change && (motion == "w" || motion == "W") {
        let mut word_cursor = cursor;
        for _ in 0..(count - 1) {
            word_cursor = if motion == "w" {
                motions::next_vim_word(text, word_cursor)
            } else {
                motions::next_word_big(text, word_cursor)
            };
        }
        let word_end = if motion == "w" {
            motions::end_of_vim_word(text, word_cursor)
        } else {
            motions::end_of_word_big(text, word_cursor)
        };
        to = next_offset(text, word_end);
    } else if is_linewise_motion(motion) {
        linewise = true;
        let next_newline = text[to..].find('\n');
        match next_newline {
            None => {
                to = text.len();
                if from > 0 && text.as_bytes()[from - 1] == b'\n' {
                    from -= 1;
                }
            }
            Some(pos) => {
                to = to + pos + 1;
            }
        }
    } else if is_inclusive_motion(motion) && cursor <= target {
        to = next_offset(text, to);
    }

    OperatorRange { from, to, linewise }
}

fn get_operator_range_for_find(text: &str, cursor: usize, target: usize) -> OperatorRange {
    let from = cursor.min(target);
    let max_offset = cursor.max(target);
    let to = next_offset(text, max_offset);
    OperatorRange {
        from,
        to,
        linewise: false,
    }
}

fn apply_operator(op: Operator, from: usize, to: usize, st: &mut EditState, linewise: bool) {
    let to = to.min(st.text.len());
    let mut content = st.text[from..to].to_string();
    if linewise && !content.ends_with('\n') {
        content.push('\n');
    }
    st.set_register(content, linewise);

    match op {
        Operator::Yank => {
            st.set_offset(from);
        }
        Operator::Delete => {
            let new_text = format!("{}{}", &st.text[..from], &st.text[to..]);
            let max_off = if new_text.is_empty() {
                0
            } else {
                new_text.len().saturating_sub(last_char_len(&new_text))
            };
            st.set_offset(from.min(max_off));
            st.text = new_text;
        }
        Operator::Change => {
            let new_text = format!("{}{}", &st.text[..from], &st.text[to..]);
            st.text = new_text;
            st.enter_insert(from);
        }
    }
}

/// Calculate the byte offset of a line's start position (for &str slices).
fn get_line_start_offset_strs(lines: &[&str], line_index: usize) -> usize {
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == line_index {
            return offset;
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

/// Same but for owned strings.
fn get_line_start_offset_from_parts(lines: &[String], line_index: usize) -> usize {
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == line_index {
            return offset;
        }
        offset += line.len() + 1;
    }
    offset
}

/// Get the length of the last character in a string.
fn last_char_len(s: &str) -> usize {
    s.chars().last().map(|c| c.len_utf8()).unwrap_or(1)
}
