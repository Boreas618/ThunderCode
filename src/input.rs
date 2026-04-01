//! Input reading and classification for the REPL.
//!
//! Reads user input character-by-character from crossterm events, supporting
//! line editing (backspace, Ctrl-U, Ctrl-W), and classifies complete lines
//! into messages, commands, or control actions.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::watch;

/// Classification of a completed input line.
#[derive(Debug)]
pub enum InputAction {
    /// Empty line (user just pressed Enter).
    Empty,
    /// Ctrl+D pressed: the user wants to exit.
    Exit,
    /// Ctrl+C pressed: interrupt the current operation.
    Interrupt,
    /// A slash command (name without the leading `/`, plus the rest of the line).
    Command { name: String, args: String },
    /// A regular message to send to the model.
    Message(String),
}

/// Read a full line of user input, handling basic line-editing keys.
///
/// Returns the completed [`InputAction`] when the user presses Enter, Ctrl+C,
/// or Ctrl+D.
///
/// `abort_rx` can signal that the prompt should be interrupted externally
/// (e.g. if a background operation completes while waiting for input).
pub async fn read_user_input(
    abort_rx: &mut watch::Receiver<bool>,
) -> io::Result<InputAction> {
    let mut buffer = String::new();
    let mut cursor_pos: usize = 0;

    loop {
        // Poll for crossterm events with a short timeout so we can also
        // check the abort channel.
        let has_event = tokio::task::block_in_place(|| {
            event::poll(Duration::from_millis(50))
        })?;

        // Check abort signal.
        if *abort_rx.borrow() {
            return Ok(InputAction::Interrupt);
        }

        if !has_event {
            continue;
        }

        let ev = tokio::task::block_in_place(|| event::read())?;

        match ev {
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => {
                // Ctrl+D on empty buffer = exit.
                if code == KeyCode::Char('d') && modifiers.contains(KeyModifiers::CONTROL) {
                    if buffer.is_empty() {
                        return Ok(InputAction::Exit);
                    }
                    // On non-empty buffer, Ctrl+D deletes char under cursor
                    // (like Unix line discipline). We just ignore for simplicity.
                    continue;
                }

                // Ctrl+C = interrupt.
                if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                    // Print ^C and newline to match terminal convention.
                    let mut stdout = io::stdout();
                    let _ = write!(stdout, "^C\r\n");
                    let _ = stdout.flush();
                    return Ok(InputAction::Interrupt);
                }

                // Ctrl+U = kill to beginning of line.
                if code == KeyCode::Char('u') && modifiers.contains(KeyModifiers::CONTROL) {
                    if cursor_pos > 0 {
                        let removed = cursor_pos;
                        buffer.drain(..cursor_pos);
                        cursor_pos = 0;
                        redraw_line(&buffer, cursor_pos, removed)?;
                    }
                    continue;
                }

                // Ctrl+W = delete word backward.
                if code == KeyCode::Char('w') && modifiers.contains(KeyModifiers::CONTROL) {
                    if cursor_pos > 0 {
                        let old_pos = cursor_pos;
                        // Skip trailing whitespace.
                        while cursor_pos > 0
                            && buffer.as_bytes().get(cursor_pos - 1) == Some(&b' ')
                        {
                            cursor_pos -= 1;
                        }
                        // Delete word chars.
                        while cursor_pos > 0
                            && buffer.as_bytes().get(cursor_pos - 1) != Some(&b' ')
                        {
                            cursor_pos -= 1;
                        }
                        buffer.drain(cursor_pos..old_pos);
                        redraw_line(&buffer, cursor_pos, old_pos - cursor_pos)?;
                    }
                    continue;
                }

                // Ctrl+A = beginning of line.
                if code == KeyCode::Char('a') && modifiers.contains(KeyModifiers::CONTROL) {
                    if cursor_pos > 0 {
                        let mut stdout = io::stdout();
                        let _ = write!(stdout, "\x1b[{}D", cursor_pos);
                        let _ = stdout.flush();
                        cursor_pos = 0;
                    }
                    continue;
                }

                // Ctrl+E = end of line.
                if code == KeyCode::Char('e') && modifiers.contains(KeyModifiers::CONTROL) {
                    if cursor_pos < buffer.len() {
                        let move_right = buffer.len() - cursor_pos;
                        let mut stdout = io::stdout();
                        let _ = write!(stdout, "\x1b[{}C", move_right);
                        let _ = stdout.flush();
                        cursor_pos = buffer.len();
                    }
                    continue;
                }

                match code {
                    KeyCode::Enter => {
                        // Print newline.
                        let mut stdout = io::stdout();
                        let _ = write!(stdout, "\r\n");
                        let _ = stdout.flush();

                        let trimmed = buffer.trim().to_string();
                        if trimmed.is_empty() {
                            return Ok(InputAction::Empty);
                        }
                        return Ok(classify_input(&trimmed));
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            buffer.remove(cursor_pos);
                            // Redraw the line from cursor position forward.
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "\x08"); // move back
                            // Write rest of buffer + space to erase last char.
                            let _ = write!(stdout, "{} ", &buffer[cursor_pos..]);
                            // Move cursor back to position.
                            let move_back = buffer.len() - cursor_pos + 1;
                            if move_back > 0 {
                                let _ = write!(stdout, "\x1b[{}D", move_back);
                            }
                            let _ = stdout.flush();
                        }
                    }
                    KeyCode::Delete => {
                        if cursor_pos < buffer.len() {
                            buffer.remove(cursor_pos);
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "{} ", &buffer[cursor_pos..]);
                            let move_back = buffer.len() - cursor_pos + 1;
                            if move_back > 0 {
                                let _ = write!(stdout, "\x1b[{}D", move_back);
                            }
                            let _ = stdout.flush();
                        }
                    }
                    KeyCode::Left => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "\x1b[1D");
                            let _ = stdout.flush();
                        }
                    }
                    KeyCode::Right => {
                        if cursor_pos < buffer.len() {
                            cursor_pos += 1;
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "\x1b[1C");
                            let _ = stdout.flush();
                        }
                    }
                    KeyCode::Home => {
                        if cursor_pos > 0 {
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "\x1b[{}D", cursor_pos);
                            let _ = stdout.flush();
                            cursor_pos = 0;
                        }
                    }
                    KeyCode::End => {
                        if cursor_pos < buffer.len() {
                            let move_right = buffer.len() - cursor_pos;
                            let mut stdout = io::stdout();
                            let _ = write!(stdout, "\x1b[{}C", move_right);
                            let _ = stdout.flush();
                            cursor_pos = buffer.len();
                        }
                    }
                    KeyCode::Char(c) => {
                        buffer.insert(cursor_pos, c);
                        cursor_pos += 1;
                        let mut stdout = io::stdout();
                        // Write from cursor to end.
                        let _ = write!(stdout, "{}", &buffer[cursor_pos - 1..]);
                        // Move cursor back to correct position.
                        let move_back = buffer.len() - cursor_pos;
                        if move_back > 0 {
                            let _ = write!(stdout, "\x1b[{}D", move_back);
                        }
                        let _ = stdout.flush();
                    }
                    _ => {}
                }
            }
            Event::Paste(text) => {
                // Bracketed paste: insert the entire text at cursor.
                buffer.insert_str(cursor_pos, &text);
                cursor_pos += text.len();
                let mut stdout = io::stdout();
                let _ = write!(stdout, "{}", &buffer[cursor_pos - text.len()..]);
                let move_back = buffer.len() - cursor_pos;
                if move_back > 0 {
                    let _ = write!(stdout, "\x1b[{}D", move_back);
                }
                let _ = stdout.flush();
            }
            _ => {}
        }
    }
}

/// Classify a non-empty trimmed input string into an [`InputAction`].
pub fn classify_input(input: &str) -> InputAction {
    if input.starts_with('/') {
        let without_slash = &input[1..];
        let (name, args) = match without_slash.split_once(char::is_whitespace) {
            Some((n, a)) => (n.to_string(), a.trim().to_string()),
            None => (without_slash.to_string(), String::new()),
        };
        // Special shorthand: /exit and /quit.
        if name == "exit" || name == "quit" {
            return InputAction::Exit;
        }
        InputAction::Command { name, args }
    } else {
        InputAction::Message(input.to_string())
    }
}

/// Helper: redraw the current line after a destructive edit.
fn redraw_line(buffer: &str, cursor_pos: usize, _chars_removed: usize) -> io::Result<()> {
    let mut stdout = io::stdout();
    // Move to beginning of input (after prompt) then clear to end of line.
    // We use carriage-return + rewrite approach.
    let _ = write!(stdout, "\r\x1b[2K");
    // Rewrite prompt + buffer.
    let _ = write!(stdout, "\x1b[1;36m> \x1b[0m{}", buffer);
    // Move cursor to correct position.
    let move_back = buffer.len() - cursor_pos;
    if move_back > 0 {
        let _ = write!(stdout, "\x1b[{}D", move_back);
    }
    stdout.flush()?;
    Ok(())
}
