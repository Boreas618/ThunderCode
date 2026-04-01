//! Style types, ANSI code representation, and color model.
//!
//! Mirrors the TypeScript `TextStyles` / `Color` types from the ref and
//! provides SGR attribute constants for the style pool.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// Terminal color representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// Standard named color (0-15).
    Named(NamedColor),
    /// ANSI 256-color palette index.
    Ansi256(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

/// Named terminal colors (standard 8 + bright 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl NamedColor {
    /// SGR foreground code (30-37, 90-97).
    pub fn fg_code(&self) -> u8 {
        match self {
            Self::Black => 30,
            Self::Red => 31,
            Self::Green => 32,
            Self::Yellow => 33,
            Self::Blue => 34,
            Self::Magenta => 35,
            Self::Cyan => 36,
            Self::White => 37,
            Self::BrightBlack => 90,
            Self::BrightRed => 91,
            Self::BrightGreen => 92,
            Self::BrightYellow => 93,
            Self::BrightBlue => 94,
            Self::BrightMagenta => 95,
            Self::BrightCyan => 96,
            Self::BrightWhite => 97,
        }
    }

    /// SGR background code (40-47, 100-107).
    pub fn bg_code(&self) -> u8 {
        self.fg_code() + 10
    }
}

// ---------------------------------------------------------------------------
// TextStyle (runtime state)
// ---------------------------------------------------------------------------

/// Complete text style state as tracked during SGR parsing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Style {
    pub fg_color: Option<Color>,
    pub bg_color: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub overline: bool,
    pub hidden: bool,
    pub blink: bool,
}

/// Structured text styling properties applied to DOM text nodes.
/// Colors are raw values -- theme resolution happens at the component layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStyles {
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub dim: Option<bool>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub inverse: Option<bool>,
}

// ---------------------------------------------------------------------------
// AnsiCode (pool element)
// ---------------------------------------------------------------------------

/// A single ANSI style code with its end/reset code.
///
/// Stored in the `StylePool` as part of a style definition.
/// `end_code` is used to categorize the code (e.g., whether it affects
/// background, underline, etc.) and to generate transitions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnsiCode {
    pub code: String,
    pub end_code: String,
}

impl AnsiCode {
    pub fn new(code: &str, end_code: &str) -> Self {
        Self {
            code: code.into(),
            end_code: end_code.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Style -> AnsiCode conversion
// ---------------------------------------------------------------------------

impl Style {
    /// Convert this style into a list of `AnsiCode` values for pool interning.
    pub fn to_ansi_codes(&self) -> Vec<AnsiCode> {
        let mut codes = Vec::new();
        if self.bold {
            codes.push(AnsiCode::new("\x1b[1m", "\x1b[22m"));
        }
        if self.dim {
            codes.push(AnsiCode::new("\x1b[2m", "\x1b[22m"));
        }
        if self.italic {
            codes.push(AnsiCode::new("\x1b[3m", "\x1b[23m"));
        }
        if self.underline {
            codes.push(AnsiCode::new("\x1b[4m", "\x1b[24m"));
        }
        if self.blink {
            codes.push(AnsiCode::new("\x1b[5m", "\x1b[25m"));
        }
        if self.inverse {
            codes.push(AnsiCode::new("\x1b[7m", "\x1b[27m"));
        }
        if self.hidden {
            codes.push(AnsiCode::new("\x1b[8m", "\x1b[28m"));
        }
        if self.strikethrough {
            codes.push(AnsiCode::new("\x1b[9m", "\x1b[29m"));
        }
        if self.overline {
            codes.push(AnsiCode::new("\x1b[53m", "\x1b[55m"));
        }
        if let Some(ref color) = self.fg_color {
            codes.push(AnsiCode::new(&color_to_fg_sgr(color), "\x1b[39m"));
        }
        if let Some(ref color) = self.bg_color {
            codes.push(AnsiCode::new(&color_to_bg_sgr(color), "\x1b[49m"));
        }
        codes
    }
}

/// Generate the SGR set-foreground sequence for a color.
pub fn color_to_fg_sgr(color: &Color) -> String {
    match color {
        Color::Named(named) => format!("\x1b[{}m", named.fg_code()),
        Color::Ansi256(idx) => format!("\x1b[38;5;{}m", idx),
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
    }
}

/// Generate the SGR set-background sequence for a color.
pub fn color_to_bg_sgr(color: &Color) -> String {
    match color {
        Color::Named(named) => format!("\x1b[{}m", named.bg_code()),
        Color::Ansi256(idx) => format!("\x1b[48;5;{}m", idx),
        Color::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
    }
}

/// Parse a color string (as used in the ref implementation).
/// Formats: `"rgb(R,G,B)"`, `"#RRGGBB"`, `"ansi256(N)"`, `"ansi:colorName"`.
pub fn parse_color(s: &str) -> Option<Color> {
    if let Some(rest) = s.strip_prefix("rgb(") {
        let rest = rest.strip_suffix(')')?;
        let parts: Vec<&str> = rest.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }
    if let Some(rest) = s.strip_prefix("ansi256(") {
        let rest = rest.strip_suffix(')')?;
        let idx = rest.trim().parse().ok()?;
        return Some(Color::Ansi256(idx));
    }
    if let Some(name) = s.strip_prefix("ansi:") {
        let named = match name {
            "black" => NamedColor::Black,
            "red" => NamedColor::Red,
            "green" => NamedColor::Green,
            "yellow" => NamedColor::Yellow,
            "blue" => NamedColor::Blue,
            "magenta" => NamedColor::Magenta,
            "cyan" => NamedColor::Cyan,
            "white" => NamedColor::White,
            "blackBright" => NamedColor::BrightBlack,
            "redBright" => NamedColor::BrightRed,
            "greenBright" => NamedColor::BrightGreen,
            "yellowBright" => NamedColor::BrightYellow,
            "blueBright" => NamedColor::BrightBlue,
            "magentaBright" => NamedColor::BrightMagenta,
            "cyanBright" => NamedColor::BrightCyan,
            "whiteBright" => NamedColor::BrightWhite,
            _ => return None,
        };
        return Some(Color::Named(named));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_color_codes() {
        assert_eq!(NamedColor::Red.fg_code(), 31);
        assert_eq!(NamedColor::Red.bg_code(), 41);
        assert_eq!(NamedColor::BrightCyan.fg_code(), 96);
        assert_eq!(NamedColor::BrightCyan.bg_code(), 106);
    }

    #[test]
    fn test_color_to_sgr() {
        assert_eq!(
            color_to_fg_sgr(&Color::Named(NamedColor::Green)),
            "\x1b[32m"
        );
        assert_eq!(color_to_fg_sgr(&Color::Ansi256(42)), "\x1b[38;5;42m");
        assert_eq!(
            color_to_fg_sgr(&Color::Rgb(255, 128, 0)),
            "\x1b[38;2;255;128;0m"
        );
        assert_eq!(
            color_to_bg_sgr(&Color::Named(NamedColor::Blue)),
            "\x1b[44m"
        );
        assert_eq!(
            color_to_bg_sgr(&Color::Rgb(10, 20, 30)),
            "\x1b[48;2;10;20;30m"
        );
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(
            parse_color("rgb(255,128,0)"),
            Some(Color::Rgb(255, 128, 0))
        );
        assert_eq!(parse_color("#ff8000"), Some(Color::Rgb(255, 128, 0)));
        assert_eq!(parse_color("ansi256(42)"), Some(Color::Ansi256(42)));
        assert_eq!(
            parse_color("ansi:red"),
            Some(Color::Named(NamedColor::Red))
        );
        assert_eq!(
            parse_color("ansi:cyanBright"),
            Some(Color::Named(NamedColor::BrightCyan))
        );
        assert_eq!(parse_color("garbage"), None);
    }

    #[test]
    fn test_style_to_ansi_codes() {
        let style = Style {
            bold: true,
            fg_color: Some(Color::Named(NamedColor::Red)),
            ..Default::default()
        };
        let codes = style.to_ansi_codes();
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0].code, "\x1b[1m");
        assert_eq!(codes[1].code, "\x1b[31m");
    }
}
