//! Vim state transition table.
//!
//! This is the scannable source of truth for state transitions.
//! To understand what happens in any state, look up that state's transition function.

use crate::vim::motions::{self, resolve_motion};
use crate::vim::operators::{
    execute_indent, execute_join, execute_line_op, execute_open_line, execute_operator_find,
    execute_operator_g_motion, execute_operator_gg, execute_operator_motion,
    execute_operator_text_obj, execute_paste, execute_replace, execute_toggle_case, execute_x,
    EditState,
};
use crate::vim::types::*;

/// Main transition function. Dispatches based on current state type.
///
/// Mutates `st` in-place. Returns the new `CommandState`.
/// `on_undo` and `on_dot_repeat` are optional callbacks for those special keys.
pub fn transition(
    state: &CommandState,
    input: char,
    st: &mut EditState,
    on_undo: Option<&mut dyn FnMut()>,
    on_dot_repeat: Option<&mut dyn FnMut()>,
) -> CommandState {
    match state {
        CommandState::Idle => from_idle(input, st, on_undo, on_dot_repeat),
        CommandState::Count { digits } => {
            from_count(digits.clone(), input, st, on_undo, on_dot_repeat)
        }
        CommandState::Operator { op, count } => from_operator(*op, *count, input, st),
        CommandState::OperatorCount { op, count, digits } => {
            from_operator_count(*op, *count, digits.clone(), input, st)
        }
        CommandState::OperatorFind { op, count, find } => {
            from_operator_find(*op, *count, *find, input, st)
        }
        CommandState::OperatorTextObj { op, count, scope } => {
            from_operator_text_obj(*op, *count, *scope, input, st)
        }
        CommandState::Find { find, count } => from_find(*find, *count, input, st),
        CommandState::G { count } => from_g(*count, input, st),
        CommandState::OperatorG { op, count } => from_operator_g(*op, *count, input, st),
        CommandState::Replace { count } => from_replace(*count, input, st),
        CommandState::Indent { dir, count } => from_indent(*dir, *count, input, st),
    }
}

// ============================================================================
// Shared Input Handling
// ============================================================================

/// Handle input valid in both idle and count states.
/// Returns Some(new_state) if handled, None if not recognized.
fn handle_normal_input(
    input: char,
    count: usize,
    st: &mut EditState,
    on_undo: Option<&mut dyn FnMut()>,
    on_dot_repeat: Option<&mut dyn FnMut()>,
) -> Option<CommandState> {
    let input_str = input.to_string();

    if let Some(op) = Operator::from_key(input) {
        return Some(CommandState::Operator { op, count });
    }

    if is_simple_motion(&input_str) {
        let target = resolve_motion(&st.text, st.cursor, &input_str, count);
        st.set_offset(target);
        return Some(CommandState::Idle);
    }

    if let Some(find) = FindType::from_key(input) {
        return Some(CommandState::Find { find, count });
    }

    if input == 'g' {
        return Some(CommandState::G { count });
    }
    if input == 'r' {
        return Some(CommandState::Replace { count });
    }
    if input == '>' || input == '<' {
        return Some(CommandState::Indent { dir: input, count });
    }
    if input == '~' {
        execute_toggle_case(count, st);
        return Some(CommandState::Idle);
    }
    if input == 'x' {
        execute_x(count, st);
        return Some(CommandState::Idle);
    }
    if input == 'J' {
        execute_join(count, st);
        return Some(CommandState::Idle);
    }
    if input == 'p' || input == 'P' {
        execute_paste(input == 'p', count, st);
        return Some(CommandState::Idle);
    }
    if input == 'D' {
        execute_operator_motion(Operator::Delete, "$", 1, st);
        return Some(CommandState::Idle);
    }
    if input == 'C' {
        execute_operator_motion(Operator::Change, "$", 1, st);
        return Some(CommandState::Idle);
    }
    if input == 'Y' {
        execute_line_op(Operator::Yank, count, st);
        return Some(CommandState::Idle);
    }
    if input == 'G' {
        if count == 1 {
            let target = motions::start_of_last_line(&st.text);
            st.set_offset(target);
        } else {
            let target = motions::go_to_line(&st.text, count);
            st.set_offset(target);
        }
        return Some(CommandState::Idle);
    }
    if input == '.' {
        if let Some(on_dot) = on_dot_repeat {
            on_dot();
        }
        return Some(CommandState::Idle);
    }
    if input == ';' || input == ',' {
        execute_repeat_find(input == ',', count, st);
        return Some(CommandState::Idle);
    }
    if input == 'u' {
        if let Some(on_u) = on_undo {
            on_u();
        }
        return Some(CommandState::Idle);
    }
    if input == 'i' {
        st.enter_insert(st.cursor);
        return Some(CommandState::Idle);
    }
    if input == 'I' {
        let target = motions::first_non_blank_in_line(&st.text, st.cursor);
        st.enter_insert(target);
        return Some(CommandState::Idle);
    }
    if input == 'a' {
        let new_offset = if motions::is_at_end(&st.text, st.cursor) {
            st.cursor
        } else {
            motions::next_offset(&st.text, st.cursor)
        };
        st.enter_insert(new_offset);
        return Some(CommandState::Idle);
    }
    if input == 'A' {
        let target = motions::end_of_logical_line(&st.text, st.cursor);
        let target = motions::next_offset(&st.text, target);
        st.enter_insert(target);
        return Some(CommandState::Idle);
    }
    if input == 'o' {
        execute_open_line(OpenLineDirection::Below, st);
        return Some(CommandState::Idle);
    }
    if input == 'O' {
        execute_open_line(OpenLineDirection::Above, st);
        return Some(CommandState::Idle);
    }

    None
}

/// Handle operator input (motion, find, text object scope).
/// Returns Some(new_state) if handled, None if not recognized.
fn handle_operator_input(
    op: Operator,
    count: usize,
    input: char,
    st: &mut EditState,
) -> Option<CommandState> {
    let input_str = input.to_string();

    if let Some(scope) = TextObjScope::from_key(input) {
        return Some(CommandState::OperatorTextObj { op, count, scope });
    }

    if let Some(find) = FindType::from_key(input) {
        return Some(CommandState::OperatorFind { op, count, find });
    }

    if is_simple_motion(&input_str) {
        execute_operator_motion(op, &input_str, count, st);
        return Some(CommandState::Idle);
    }

    if input == 'G' {
        execute_operator_g_motion(op, count, st);
        return Some(CommandState::Idle);
    }

    if input == 'g' {
        return Some(CommandState::OperatorG { op, count });
    }

    None
}

// ============================================================================
// Transition Functions - One per state type
// ============================================================================

fn from_idle(
    input: char,
    st: &mut EditState,
    on_undo: Option<&mut dyn FnMut()>,
    on_dot_repeat: Option<&mut dyn FnMut()>,
) -> CommandState {
    // 0 is line-start motion, not a count prefix
    if input.is_ascii_digit() && input != '0' {
        return CommandState::Count {
            digits: input.to_string(),
        };
    }
    if input == '0' {
        let target = motions::start_of_logical_line(&st.text, st.cursor);
        st.set_offset(target);
        return CommandState::Idle;
    }

    handle_normal_input(input, 1, st, on_undo, on_dot_repeat).unwrap_or(CommandState::Idle)
}

fn from_count(
    digits: String,
    input: char,
    st: &mut EditState,
    on_undo: Option<&mut dyn FnMut()>,
    on_dot_repeat: Option<&mut dyn FnMut()>,
) -> CommandState {
    if input.is_ascii_digit() {
        let new_digits = format!("{}{}", digits, input);
        let count = new_digits
            .parse::<usize>()
            .unwrap_or(1)
            .min(MAX_VIM_COUNT);
        return CommandState::Count {
            digits: count.to_string(),
        };
    }

    let count = digits.parse::<usize>().unwrap_or(1);
    handle_normal_input(input, count, st, on_undo, on_dot_repeat).unwrap_or(CommandState::Idle)
}

fn from_operator(
    op: Operator,
    count: usize,
    input: char,
    st: &mut EditState,
) -> CommandState {
    // dd, cc, yy = line operation
    if input == op.key() {
        execute_line_op(op, count, st);
        return CommandState::Idle;
    }

    if input.is_ascii_digit() {
        return CommandState::OperatorCount {
            op,
            count,
            digits: input.to_string(),
        };
    }

    handle_operator_input(op, count, input, st).unwrap_or(CommandState::Idle)
}

fn from_operator_count(
    op: Operator,
    count: usize,
    digits: String,
    input: char,
    st: &mut EditState,
) -> CommandState {
    if input.is_ascii_digit() {
        let new_digits = format!("{}{}", digits, input);
        let parsed = new_digits
            .parse::<usize>()
            .unwrap_or(1)
            .min(MAX_VIM_COUNT);
        return CommandState::OperatorCount {
            op,
            count,
            digits: parsed.to_string(),
        };
    }

    let motion_count = digits.parse::<usize>().unwrap_or(1);
    let effective_count = count * motion_count;
    handle_operator_input(op, effective_count, input, st).unwrap_or(CommandState::Idle)
}

fn from_operator_find(
    op: Operator,
    count: usize,
    find: FindType,
    input: char,
    st: &mut EditState,
) -> CommandState {
    execute_operator_find(op, find, input, count, st);
    CommandState::Idle
}

fn from_operator_text_obj(
    op: Operator,
    count: usize,
    scope: TextObjScope,
    input: char,
    st: &mut EditState,
) -> CommandState {
    if is_text_obj_type(input) {
        execute_operator_text_obj(op, scope, input, count, st);
        return CommandState::Idle;
    }
    CommandState::Idle
}

fn from_find(find: FindType, count: usize, input: char, st: &mut EditState) -> CommandState {
    let result = motions::find_character(&st.text, st.cursor, input, find, count);
    if let Some(offset) = result {
        st.set_offset(offset);
        st.set_last_find(find, input);
    }
    CommandState::Idle
}

fn from_g(count: usize, input: char, st: &mut EditState) -> CommandState {
    if input == 'j' || input == 'k' {
        let motion = format!("g{}", input);
        let target = resolve_motion(&st.text, st.cursor, &motion, count);
        st.set_offset(target);
        return CommandState::Idle;
    }
    if input == 'g' {
        if count > 1 {
            let lines: Vec<&str> = st.text.split('\n').collect();
            let target_line = (count - 1).min(lines.len() - 1);
            let mut offset = 0;
            for i in 0..target_line {
                offset += lines[i].len() + 1;
            }
            st.set_offset(offset);
        } else {
            st.set_offset(0);
        }
        return CommandState::Idle;
    }
    CommandState::Idle
}

fn from_operator_g(
    op: Operator,
    count: usize,
    input: char,
    st: &mut EditState,
) -> CommandState {
    if input == 'j' || input == 'k' {
        let motion = format!("g{}", input);
        execute_operator_motion(op, &motion, count, st);
        return CommandState::Idle;
    }
    if input == 'g' {
        execute_operator_gg(op, count, st);
        return CommandState::Idle;
    }
    CommandState::Idle
}

fn from_replace(count: usize, input: char, st: &mut EditState) -> CommandState {
    if input.is_control() {
        return CommandState::Idle;
    }
    execute_replace(input, count, st);
    CommandState::Idle
}

fn from_indent(dir: char, count: usize, input: char, st: &mut EditState) -> CommandState {
    if input == dir {
        execute_indent(dir, count, st);
        return CommandState::Idle;
    }
    CommandState::Idle
}

// ============================================================================
// Helper functions for special commands
// ============================================================================

fn execute_repeat_find(reverse: bool, count: usize, st: &mut EditState) {
    let last_find = match &st.last_find {
        Some(f) => f.clone(),
        None => return,
    };

    let effective_type = if reverse {
        last_find.find_type.flipped()
    } else {
        last_find.find_type
    };

    let result = motions::find_character(&st.text, st.cursor, last_find.ch, effective_type, count);
    if let Some(offset) = result {
        st.set_offset(offset);
    }
}
