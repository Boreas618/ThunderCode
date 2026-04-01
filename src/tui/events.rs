//! Terminal event parsing: keyboard, mouse, focus, paste, resize.
//!
//! Parses raw terminal input bytes into structured events. Handles:
//! - Standard keys and escape sequences
//! - CSI-encoded keys (arrow keys, function keys, etc.)
//! - Kitty keyboard protocol (CSI u)
//! - SGR mouse events (CSI < ... M/m)
//! - Bracketed paste (CSI 200~ ... CSI 201~)
//! - Focus events (CSI I / CSI O)

use crossterm::event::{
    Event as CrosstermEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent,
    KeyEventKind, KeyModifiers, MouseButton as CtMouseButton, MouseEvent as CtMouseEvent,
    MouseEventKind as CtMouseEventKind,
};

/// Parsed terminal event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Focus(FocusEvent),
    Resize(u16, u16),
    Paste(String),
}

/// Keyboard event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
}

/// Mouse event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub button: MouseButton,
    pub kind: MouseEventKind,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
    ScrollUp,
    ScrollDown,
}

/// Focus event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    Gained,
    Lost,
}

/// Key identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    BackTab,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Null,
}

/// Modifier key flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl() -> Self {
        Self {
            control: true,
            ..Default::default()
        }
    }

    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Default::default()
        }
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    None,
}

/// Convert a crossterm event into our terminal event.
pub fn from_crossterm_event(event: CrosstermEvent) -> Option<TerminalEvent> {
    match event {
        CrosstermEvent::Key(ke) => {
            if ke.kind == KeyEventKind::Release {
                return None; // We only care about press/repeat
            }
            Some(TerminalEvent::Key(convert_key_event(ke)))
        }
        CrosstermEvent::Mouse(me) => Some(TerminalEvent::Mouse(convert_mouse_event(me))),
        CrosstermEvent::Resize(w, h) => Some(TerminalEvent::Resize(w, h)),
        CrosstermEvent::FocusGained => Some(TerminalEvent::Focus(FocusEvent::Gained)),
        CrosstermEvent::FocusLost => Some(TerminalEvent::Focus(FocusEvent::Lost)),
        CrosstermEvent::Paste(text) => Some(TerminalEvent::Paste(text)),
    }
}

fn convert_key_event(ke: CtKeyEvent) -> KeyEvent {
    let key = match ke.code {
        CtKeyCode::Char(c) => Key::Char(c),
        CtKeyCode::Enter => Key::Enter,
        CtKeyCode::Esc => Key::Escape,
        CtKeyCode::Backspace => Key::Backspace,
        CtKeyCode::Tab => Key::Tab,
        CtKeyCode::BackTab => Key::BackTab,
        CtKeyCode::Delete => Key::Delete,
        CtKeyCode::Insert => Key::Insert,
        CtKeyCode::Home => Key::Home,
        CtKeyCode::End => Key::End,
        CtKeyCode::PageUp => Key::PageUp,
        CtKeyCode::PageDown => Key::PageDown,
        CtKeyCode::Up => Key::Up,
        CtKeyCode::Down => Key::Down,
        CtKeyCode::Left => Key::Left,
        CtKeyCode::Right => Key::Right,
        CtKeyCode::F(n) => Key::F(n),
        CtKeyCode::Null => Key::Null,
        _ => Key::Null,
    };

    let modifiers = Modifiers {
        shift: ke.modifiers.contains(KeyModifiers::SHIFT),
        control: ke.modifiers.contains(KeyModifiers::CONTROL),
        alt: ke.modifiers.contains(KeyModifiers::ALT),
        super_key: ke.modifiers.contains(KeyModifiers::SUPER),
    };

    KeyEvent { key, modifiers }
}

fn convert_mouse_event(me: CtMouseEvent) -> MouseEvent {
    let (button, kind) = match me.kind {
        CtMouseEventKind::Down(btn) => (convert_button(btn), MouseEventKind::Press),
        CtMouseEventKind::Up(btn) => (convert_button(btn), MouseEventKind::Release),
        CtMouseEventKind::Drag(btn) => (convert_button(btn), MouseEventKind::Move),
        CtMouseEventKind::Moved => (MouseButton::None, MouseEventKind::Move),
        CtMouseEventKind::ScrollUp => (MouseButton::None, MouseEventKind::ScrollUp),
        CtMouseEventKind::ScrollDown => (MouseButton::None, MouseEventKind::ScrollDown),
        _ => (MouseButton::None, MouseEventKind::Move),
    };

    MouseEvent {
        x: me.column,
        y: me.row,
        button,
        kind,
    }
}

fn convert_button(btn: CtMouseButton) -> MouseButton {
    match btn {
        CtMouseButton::Left => MouseButton::Left,
        CtMouseButton::Right => MouseButton::Right,
        CtMouseButton::Middle => MouseButton::Middle,
    }
}

/// Parse raw input bytes into terminal events.
/// This is a convenience wrapper that delegates to crossterm's event parsing.
/// For direct byte parsing (e.g., in tests), use this function.
pub fn parse_input(buf: &[u8]) -> Vec<TerminalEvent> {
    // For raw byte parsing, we implement basic escape sequence detection.
    // In production, crossterm handles this via its event reader.
    let mut events = Vec::new();
    let mut i = 0;

    while i < buf.len() {
        let b = buf[i];

        if b == 0x1b {
            // ESC
            if i + 1 < buf.len() {
                match buf[i + 1] {
                    b'[' => {
                        // CSI sequence
                        if let Some((event, advance)) = parse_csi(&buf[i + 2..]) {
                            events.push(event);
                            i += 2 + advance;
                            continue;
                        }
                    }
                    b => {
                        // Alt + key
                        events.push(TerminalEvent::Key(KeyEvent {
                            key: Key::Char(b as char),
                            modifiers: Modifiers::alt(),
                        }));
                        i += 2;
                        continue;
                    }
                }
            }
            events.push(TerminalEvent::Key(KeyEvent {
                key: Key::Escape,
                modifiers: Modifiers::none(),
            }));
            i += 1;
        } else if b == 0x0d {
            // CR = Enter
            events.push(TerminalEvent::Key(KeyEvent {
                key: Key::Enter,
                modifiers: Modifiers::none(),
            }));
            i += 1;
        } else if b == 0x09 {
            // HT = Tab
            events.push(TerminalEvent::Key(KeyEvent {
                key: Key::Tab,
                modifiers: Modifiers::none(),
            }));
            i += 1;
        } else if b == 0x7f {
            // DEL = Backspace
            events.push(TerminalEvent::Key(KeyEvent {
                key: Key::Backspace,
                modifiers: Modifiers::none(),
            }));
            i += 1;
        } else if b < 0x20 {
            // Ctrl + letter
            events.push(TerminalEvent::Key(KeyEvent {
                key: Key::Char((b + 0x60) as char),
                modifiers: Modifiers::ctrl(),
            }));
            i += 1;
        } else {
            // Regular character (possibly multi-byte UTF-8)
            let s = std::str::from_utf8(&buf[i..]).unwrap_or("");
            if let Some(c) = s.chars().next() {
                events.push(TerminalEvent::Key(KeyEvent {
                    key: Key::Char(c),
                    modifiers: Modifiers::none(),
                }));
                i += c.len_utf8();
            } else {
                i += 1;
            }
        }
    }

    events
}

/// Parse a CSI sequence (after ESC [). Returns (event, bytes_consumed).
fn parse_csi(buf: &[u8]) -> Option<(TerminalEvent, usize)> {
    if buf.is_empty() {
        return None;
    }

    // Find the final byte (0x40-0x7E)
    let mut end = 0;
    while end < buf.len() && !(0x40..=0x7E).contains(&buf[end]) {
        end += 1;
    }
    if end >= buf.len() {
        return None;
    }

    let final_byte = buf[end];
    let _params = &buf[..end];

    let event = match final_byte {
        b'A' => TerminalEvent::Key(KeyEvent {
            key: Key::Up,
            modifiers: Modifiers::none(),
        }),
        b'B' => TerminalEvent::Key(KeyEvent {
            key: Key::Down,
            modifiers: Modifiers::none(),
        }),
        b'C' => TerminalEvent::Key(KeyEvent {
            key: Key::Right,
            modifiers: Modifiers::none(),
        }),
        b'D' => TerminalEvent::Key(KeyEvent {
            key: Key::Left,
            modifiers: Modifiers::none(),
        }),
        b'H' => TerminalEvent::Key(KeyEvent {
            key: Key::Home,
            modifiers: Modifiers::none(),
        }),
        b'F' => TerminalEvent::Key(KeyEvent {
            key: Key::End,
            modifiers: Modifiers::none(),
        }),
        b'Z' => TerminalEvent::Key(KeyEvent {
            key: Key::BackTab,
            modifiers: Modifiers::shift(),
        }),
        b'I' => TerminalEvent::Focus(FocusEvent::Gained),
        b'O' => TerminalEvent::Focus(FocusEvent::Lost),
        _ => return None,
    };

    Some((event, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_regular_char() {
        let events = parse_input(b"a");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Char('a'),
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_enter() {
        let events = parse_input(b"\r");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Enter,
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_ctrl_c() {
        let events = parse_input(b"\x03");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Char('c'),
                modifiers: Modifiers::ctrl(),
            })
        );
    }

    #[test]
    fn test_parse_escape() {
        let events = parse_input(b"\x1b");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Escape,
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_arrow_keys() {
        let events = parse_input(b"\x1b[A");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Up,
                modifiers: Modifiers::none(),
            })
        );

        let events = parse_input(b"\x1b[B");
        assert_eq!(events[0].clone(), TerminalEvent::Key(KeyEvent {
            key: Key::Down,
            modifiers: Modifiers::none(),
        }));

        let events = parse_input(b"\x1b[C");
        assert_eq!(events[0].clone(), TerminalEvent::Key(KeyEvent {
            key: Key::Right,
            modifiers: Modifiers::none(),
        }));

        let events = parse_input(b"\x1b[D");
        assert_eq!(events[0].clone(), TerminalEvent::Key(KeyEvent {
            key: Key::Left,
            modifiers: Modifiers::none(),
        }));
    }

    #[test]
    fn test_parse_focus_events() {
        let events = parse_input(b"\x1b[I");
        assert_eq!(events[0], TerminalEvent::Focus(FocusEvent::Gained));

        let events = parse_input(b"\x1b[O");
        assert_eq!(events[0], TerminalEvent::Focus(FocusEvent::Lost));
    }

    #[test]
    fn test_parse_backspace() {
        let events = parse_input(b"\x7f");
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Backspace,
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_tab() {
        let events = parse_input(b"\t");
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Tab,
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_alt_key() {
        let events = parse_input(b"\x1bx");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Char('x'),
                modifiers: Modifiers::alt(),
            })
        );
    }

    #[test]
    fn test_parse_multiple_inputs() {
        let events = parse_input(b"abc");
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Char('a'),
                modifiers: Modifiers::none(),
            })
        );
    }

    #[test]
    fn test_parse_unicode() {
        // UTF-8 encoded emoji
        let events = parse_input("\u{1F600}".as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TerminalEvent::Key(KeyEvent {
                key: Key::Char('\u{1F600}'),
                modifiers: Modifiers::none(),
            })
        );
    }
}
