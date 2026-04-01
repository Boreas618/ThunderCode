//! Terminal I/O escape sequences: CSI, SGR, OSC, DEC.
//!
//! Provides builders for ANSI escape sequences used in terminal rendering.
//! Matches the ref implementation's `termio/csi.ts`, `termio/sgr.ts`, and
//! `termio/ansi.ts`.

/// C0 control characters.
pub mod c0 {
    pub const ESC: char = '\x1b';
    pub const BEL: char = '\x07';
    pub const BS: char = '\x08';
    pub const HT: char = '\x09';
    pub const LF: char = '\x0a';
    pub const CR: char = '\x0d';
}

/// CSI (Control Sequence Introducer) escape sequences.
pub mod csi {
    use super::c0::ESC;

    /// CSI prefix: ESC [
    pub const CSI_PREFIX: &str = "\x1b[";

    /// Erase mode for ED/EL commands.
    #[derive(Debug, Clone, Copy)]
    pub enum EraseMode {
        /// From cursor to end.
        ToEnd = 0,
        /// From start to cursor.
        ToStart = 1,
        /// Entire display/line.
        All = 2,
        /// Scrollback buffer (ED only).
        Scrollback = 3,
    }

    /// Build a CSI sequence with numeric params and a final byte.
    fn csi_seq(params: &[u32], final_byte: char) -> String {
        if params.is_empty() {
            return format!("{}{}{}", ESC, '[', final_byte);
        }
        let params_str: Vec<String> = params.iter().map(|p| p.to_string()).collect();
        format!("{}[{}{}", ESC, params_str.join(";"), final_byte)
    }

    /// Move cursor up n lines (CSI n A).
    pub fn cursor_up(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'A')
    }

    /// Move cursor down n lines (CSI n B).
    pub fn cursor_down(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'B')
    }

    /// Move cursor forward n columns (CSI n C).
    pub fn cursor_forward(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'C')
    }

    /// Move cursor back n columns (CSI n D).
    pub fn cursor_back(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'D')
    }

    /// Move cursor to column n (1-indexed) (CSI n G).
    pub fn cursor_to_column(col: u32) -> String {
        csi_seq(&[col], 'G')
    }

    /// Move cursor to row, col (1-indexed) (CSI row ; col H).
    pub fn cursor_position(row: u32, col: u32) -> String {
        csi_seq(&[row, col], 'H')
    }

    /// Cursor home (CSI H).
    pub const CURSOR_HOME: &str = "\x1b[H";

    /// Move cursor to column 1 (CSI G).
    pub const CURSOR_LEFT: &str = "\x1b[G";

    /// Move cursor relative to current position.
    pub fn cursor_move(x: i32, y: i32) -> String {
        let mut result = String::new();
        if x < 0 {
            result.push_str(&cursor_back(-x as u32));
        } else if x > 0 {
            result.push_str(&cursor_forward(x as u32));
        }
        if y < 0 {
            result.push_str(&cursor_up(-y as u32));
        } else if y > 0 {
            result.push_str(&cursor_down(y as u32));
        }
        result
    }

    /// Save cursor position (CSI s).
    pub const CURSOR_SAVE: &str = "\x1b[s";

    /// Restore cursor position (CSI u).
    pub const CURSOR_RESTORE: &str = "\x1b[u";

    /// Erase from cursor to end of line (CSI K).
    pub fn erase_to_end_of_line() -> String {
        format!("{}[K", ESC)
    }

    /// Erase entire line (CSI 2 K).
    pub fn erase_line() -> String {
        csi_seq(&[2], 'K')
    }

    /// Erase entire line (constant form).
    pub const ERASE_LINE: &str = "\x1b[2K";

    /// Erase in display with given mode (CSI mode J).
    pub fn erase_display(mode: EraseMode) -> String {
        csi_seq(&[mode as u32], 'J')
    }

    /// Erase from cursor to end of screen (CSI J).
    pub fn erase_to_end_of_screen() -> String {
        format!("{}[J", ESC)
    }

    /// Erase entire screen (CSI 2 J).
    pub fn erase_screen() -> String {
        csi_seq(&[2], 'J')
    }

    /// Erase entire screen (constant form).
    pub const ERASE_SCREEN: &str = "\x1b[2J";

    /// Erase scrollback buffer (CSI 3 J).
    pub const ERASE_SCROLLBACK: &str = "\x1b[3J";

    /// Erase n lines from cursor, moving up.
    pub fn erase_lines(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        let mut result = String::new();
        for i in 0..n {
            result.push_str(ERASE_LINE);
            if i < n - 1 {
                result.push_str(&cursor_up(1));
            }
        }
        result.push_str(CURSOR_LEFT);
        result
    }

    /// Scroll up n lines (CSI n S).
    pub fn scroll_up(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'S')
    }

    /// Scroll down n lines (CSI n T).
    pub fn scroll_down(n: u32) -> String {
        if n == 0 {
            return String::new();
        }
        csi_seq(&[n], 'T')
    }

    /// Set scroll region (DECSTBM, CSI top;bottom r). 1-indexed, inclusive.
    pub fn set_scroll_region(top: u32, bottom: u32) -> String {
        csi_seq(&[top, bottom], 'r')
    }

    /// Reset scroll region to full screen (CSI r).
    pub const RESET_SCROLL_REGION: &str = "\x1b[r";

    /// Bracketed paste start marker (input).
    pub const PASTE_START: &str = "\x1b[200~";

    /// Bracketed paste end marker (input).
    pub const PASTE_END: &str = "\x1b[201~";

    /// Focus in marker (input).
    pub const FOCUS_IN: &str = "\x1b[I";

    /// Focus out marker (input).
    pub const FOCUS_OUT: &str = "\x1b[O";

    /// Enable Kitty keyboard protocol (CSI > 1 u).
    pub const ENABLE_KITTY_KEYBOARD: &str = "\x1b[>1u";

    /// Disable Kitty keyboard protocol (CSI < u).
    pub const DISABLE_KITTY_KEYBOARD: &str = "\x1b[<u";

    /// Enable xterm modifyOtherKeys level 2.
    pub const ENABLE_MODIFY_OTHER_KEYS: &str = "\x1b[>4;2m";

    /// Disable xterm modifyOtherKeys.
    pub const DISABLE_MODIFY_OTHER_KEYS: &str = "\x1b[>4m";
}

/// SGR (Select Graphic Rendition) sequences.
pub mod sgr {
    use crate::tui::style::{Color, NamedColor, Style};

    /// Reset all attributes (SGR 0).
    pub fn reset() -> String {
        "\x1b[0m".into()
    }

    /// Bold (SGR 1).
    pub fn bold() -> String {
        "\x1b[1m".into()
    }

    /// Dim (SGR 2).
    pub fn dim() -> String {
        "\x1b[2m".into()
    }

    /// Italic (SGR 3).
    pub fn italic() -> String {
        "\x1b[3m".into()
    }

    /// Underline (SGR 4).
    pub fn underline() -> String {
        "\x1b[4m".into()
    }

    /// Inverse (SGR 7).
    pub fn inverse() -> String {
        "\x1b[7m".into()
    }

    /// Strikethrough (SGR 9).
    pub fn strikethrough() -> String {
        "\x1b[9m".into()
    }

    /// Overline (SGR 53).
    pub fn overline() -> String {
        "\x1b[53m".into()
    }

    /// Set foreground color.
    pub fn fg_color(color: &Color) -> String {
        crate::tui::style::color_to_fg_sgr(color)
    }

    /// Set background color.
    pub fn bg_color(color: &Color) -> String {
        crate::tui::style::color_to_bg_sgr(color)
    }

    /// Convert a full Style to an SGR string.
    pub fn style_to_sgr(style: &Style) -> String {
        let mut parts = Vec::new();
        if style.bold {
            parts.push("1".to_string());
        }
        if style.dim {
            parts.push("2".to_string());
        }
        if style.italic {
            parts.push("3".to_string());
        }
        if style.underline {
            parts.push("4".to_string());
        }
        if style.blink {
            parts.push("5".to_string());
        }
        if style.inverse {
            parts.push("7".to_string());
        }
        if style.hidden {
            parts.push("8".to_string());
        }
        if style.strikethrough {
            parts.push("9".to_string());
        }
        if style.overline {
            parts.push("53".to_string());
        }
        if let Some(ref color) = style.fg_color {
            parts.push(color_params(color, false));
        }
        if let Some(ref color) = style.bg_color {
            parts.push(color_params(color, true));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", parts.join(";"))
        }
    }

    /// Compute the diff between two styles as an ANSI string.
    /// Tries to emit minimal SGR codes to transition from `from` to `to`.
    pub fn diff_styles(from: &Style, to: &Style) -> String {
        if from == to {
            return String::new();
        }

        // If going to default, just reset
        if *to == Style::default() {
            return reset();
        }

        // If coming from default, just apply target
        if *from == Style::default() {
            return style_to_sgr(to);
        }

        let mut codes = Vec::new();
        let mut need_reset = false;

        // Attributes that can only be turned off by their specific reset code
        // or a full reset. Check if any attribute was on and is now off.
        if from.bold && !to.bold {
            need_reset = true;
        }
        if from.dim && !to.dim {
            need_reset = true;
        }

        if need_reset {
            // Reset and re-apply everything
            return format!("{}{}", reset(), style_to_sgr(to));
        }

        // Incremental diff
        if !from.bold && to.bold {
            codes.push("1");
        }
        if !from.dim && to.dim {
            codes.push("2");
        }
        if !from.italic && to.italic {
            codes.push("3");
        }
        if from.italic && !to.italic {
            codes.push("23");
        }
        if !from.underline && to.underline {
            codes.push("4");
        }
        if from.underline && !to.underline {
            codes.push("24");
        }
        if !from.inverse && to.inverse {
            codes.push("7");
        }
        if from.inverse && !to.inverse {
            codes.push("27");
        }
        if !from.strikethrough && to.strikethrough {
            codes.push("9");
        }
        if from.strikethrough && !to.strikethrough {
            codes.push("29");
        }
        if !from.overline && to.overline {
            codes.push("53");
        }
        if from.overline && !to.overline {
            codes.push("55");
        }

        let mut result = if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        };

        if from.fg_color != to.fg_color {
            match &to.fg_color {
                Some(c) => result.push_str(&crate::tui::style::color_to_fg_sgr(c)),
                None => result.push_str("\x1b[39m"),
            }
        }
        if from.bg_color != to.bg_color {
            match &to.bg_color {
                Some(c) => result.push_str(&crate::tui::style::color_to_bg_sgr(c)),
                None => result.push_str("\x1b[49m"),
            }
        }

        result
    }

    fn color_params(color: &Color, is_bg: bool) -> String {
        let base = if is_bg { 40 } else { 30 };
        match color {
            Color::Named(named) => {
                let code = if is_bg {
                    named.bg_code()
                } else {
                    named.fg_code()
                };
                code.to_string()
            }
            Color::Ansi256(idx) => format!("{};5;{}", base + 8, idx),
            Color::Rgb(r, g, b) => format!("{};2;{};{};{}", base + 8, r, g, b),
        }
    }

    /// Apply SGR parameter string to a mutable Style. Handles the full range
    /// of SGR codes including extended color (38/48 with 256 and RGB).
    pub fn apply_sgr(params: &str, style: &mut Style) {
        let nums: Vec<Option<u32>> = if params.is_empty() {
            vec![Some(0)]
        } else {
            params
                .split(';')
                .map(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        s.parse().ok()
                    }
                })
                .collect()
        };

        let mut i = 0;
        while i < nums.len() {
            let code = nums[i].unwrap_or(0);
            match code {
                0 => *style = Style::default(),
                1 => style.bold = true,
                2 => style.dim = true,
                3 => style.italic = true,
                4 => style.underline = true,
                5 | 6 => style.blink = true,
                7 => style.inverse = true,
                8 => style.hidden = true,
                9 => style.strikethrough = true,
                22 => {
                    style.bold = false;
                    style.dim = false;
                }
                23 => style.italic = false,
                24 => style.underline = false,
                25 => style.blink = false,
                27 => style.inverse = false,
                28 => style.hidden = false,
                29 => style.strikethrough = false,
                53 => style.overline = true,
                55 => style.overline = false,
                30..=37 => {
                    style.fg_color = Some(Color::Named(named_from_offset(code - 30)));
                }
                39 => style.fg_color = None,
                40..=47 => {
                    style.bg_color = Some(Color::Named(named_from_offset(code - 40)));
                }
                49 => style.bg_color = None,
                90..=97 => {
                    style.fg_color = Some(Color::Named(named_from_offset(code - 90 + 8)));
                }
                100..=107 => {
                    style.bg_color = Some(Color::Named(named_from_offset(code - 100 + 8)));
                }
                38 => {
                    if let Some(color) = parse_extended_color(&nums, i) {
                        style.fg_color = Some(color.0);
                        i += color.1;
                        continue;
                    }
                }
                48 => {
                    if let Some(color) = parse_extended_color(&nums, i) {
                        style.bg_color = Some(color.0);
                        i += color.1;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn named_from_offset(offset: u32) -> NamedColor {
        match offset {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            7 => NamedColor::White,
            8 => NamedColor::BrightBlack,
            9 => NamedColor::BrightRed,
            10 => NamedColor::BrightGreen,
            11 => NamedColor::BrightYellow,
            12 => NamedColor::BrightBlue,
            13 => NamedColor::BrightMagenta,
            14 => NamedColor::BrightCyan,
            15 => NamedColor::BrightWhite,
            _ => NamedColor::White,
        }
    }

    /// Parse extended color (256-color or RGB) from SGR params at position `idx`.
    /// Returns `(Color, advance)` where advance is how many params to skip.
    fn parse_extended_color(
        nums: &[Option<u32>],
        idx: usize,
    ) -> Option<(Color, usize)> {
        let next = nums.get(idx + 1).copied().flatten()?;
        if next == 5 {
            // 256-color: 38;5;N
            let n = nums.get(idx + 2).copied().flatten()?;
            Some((Color::Ansi256(n as u8), 3))
        } else if next == 2 {
            // RGB: 38;2;R;G;B
            let r = nums.get(idx + 2).copied().flatten()?;
            let g = nums.get(idx + 3).copied().flatten()?;
            let b = nums.get(idx + 4).copied().flatten()?;
            Some((Color::Rgb(r as u8, g as u8, b as u8), 5))
        } else {
            None
        }
    }
}

/// OSC (Operating System Command) sequences.
pub mod osc {
    use super::c0::{BEL, ESC};

    /// Create a hyperlink (OSC 8).
    pub fn hyperlink(url: &str, id: Option<&str>) -> String {
        let params = match id {
            Some(id) => format!("id={}", id),
            None => String::new(),
        };
        format!("{}]8;{};{}{}", ESC, params, url, BEL)
    }

    /// End a hyperlink.
    pub fn hyperlink_end() -> String {
        format!("{}]8;;{}", ESC, BEL)
    }

    /// Copy text to clipboard (OSC 52).
    pub fn clipboard(data: &str) -> String {
        format!("{}]52;c;{}{}", ESC, data, BEL)
    }

    /// Set terminal title (OSC 2).
    pub fn set_title(title: &str) -> String {
        format!("{}]2;{}{}", ESC, title, BEL)
    }
}

/// DEC private mode sequences.
pub mod dec {
    use super::c0::ESC;

    /// Enable alternate screen buffer.
    pub fn enable_alt_screen() -> String {
        format!("{}[?1049h", ESC)
    }

    /// Disable alternate screen buffer.
    pub fn disable_alt_screen() -> String {
        format!("{}[?1049l", ESC)
    }

    /// Hide cursor.
    pub fn hide_cursor() -> String {
        format!("{}[?25l", ESC)
    }

    /// Show cursor.
    pub fn show_cursor() -> String {
        format!("{}[?25h", ESC)
    }

    /// Enable mouse tracking (SGR mode).
    pub fn enable_mouse() -> String {
        format!("{}[?1000h{}[?1006h", ESC, ESC)
    }

    /// Disable mouse tracking.
    pub fn disable_mouse() -> String {
        format!("{}[?1000l{}[?1006l", ESC, ESC)
    }

    /// Enable focus events.
    pub fn enable_focus() -> String {
        format!("{}[?1004h", ESC)
    }

    /// Disable focus events.
    pub fn disable_focus() -> String {
        format!("{}[?1004l", ESC)
    }

    /// Enable bracketed paste mode.
    pub fn enable_bracketed_paste() -> String {
        format!("{}[?2004h", ESC)
    }

    /// Disable bracketed paste mode.
    pub fn disable_bracketed_paste() -> String {
        format!("{}[?2004l", ESC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::{Color, NamedColor, Style};

    #[test]
    fn test_csi_cursor_movement() {
        assert_eq!(csi::cursor_up(3), "\x1b[3A");
        assert_eq!(csi::cursor_down(1), "\x1b[1B");
        assert_eq!(csi::cursor_forward(5), "\x1b[5C");
        assert_eq!(csi::cursor_back(2), "\x1b[2D");
        assert_eq!(csi::cursor_up(0), "");
        assert_eq!(csi::cursor_position(1, 1), "\x1b[1;1H");
    }

    #[test]
    fn test_csi_erase() {
        assert_eq!(csi::erase_line(), "\x1b[2K");
        assert_eq!(csi::erase_screen(), "\x1b[2J");
        assert_eq!(
            csi::erase_display(csi::EraseMode::Scrollback),
            "\x1b[3J"
        );
    }

    #[test]
    fn test_csi_scroll() {
        assert_eq!(csi::scroll_up(5), "\x1b[5S");
        assert_eq!(csi::scroll_down(3), "\x1b[3T");
        assert_eq!(csi::set_scroll_region(1, 24), "\x1b[1;24r");
    }

    #[test]
    fn test_sgr_style_to_sgr() {
        let style = Style {
            bold: true,
            fg_color: Some(Color::Named(NamedColor::Red)),
            ..Default::default()
        };
        let sgr = sgr::style_to_sgr(&style);
        assert!(sgr.contains("1"));
        assert!(sgr.contains("31"));
    }

    #[test]
    fn test_sgr_diff_styles() {
        let from = Style::default();
        let to = Style {
            bold: true,
            ..Default::default()
        };
        let diff = sgr::diff_styles(&from, &to);
        assert!(diff.contains("1"));

        // Same style -> empty
        assert_eq!(sgr::diff_styles(&from, &from), "");
    }

    #[test]
    fn test_sgr_apply() {
        let mut style = Style::default();
        sgr::apply_sgr("1;31", &mut style);
        assert!(style.bold);
        assert_eq!(style.fg_color, Some(Color::Named(NamedColor::Red)));

        sgr::apply_sgr("0", &mut style);
        assert!(!style.bold);
        assert!(style.fg_color.is_none());
    }

    #[test]
    fn test_sgr_apply_256_color() {
        let mut style = Style::default();
        sgr::apply_sgr("38;5;42", &mut style);
        assert_eq!(style.fg_color, Some(Color::Ansi256(42)));
    }

    #[test]
    fn test_sgr_apply_rgb() {
        let mut style = Style::default();
        sgr::apply_sgr("48;2;255;128;0", &mut style);
        assert_eq!(style.bg_color, Some(Color::Rgb(255, 128, 0)));
    }

    #[test]
    fn test_osc_hyperlink() {
        let s = osc::hyperlink("https://example.com", None);
        assert!(s.contains("8;;https://example.com"));
    }

    #[test]
    fn test_osc_set_title() {
        let s = osc::set_title("My Terminal");
        assert!(s.contains("2;My Terminal"));
    }

    #[test]
    fn test_dec_modes() {
        assert!(dec::enable_alt_screen().contains("1049h"));
        assert!(dec::disable_alt_screen().contains("1049l"));
        assert!(dec::hide_cursor().contains("25l"));
        assert!(dec::show_cursor().contains("25h"));
    }
}
