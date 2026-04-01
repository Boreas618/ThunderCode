//! Text truncation utilities: width-aware truncation, path middle-truncation, wrapping.
//!
//! Ported from ref/utils/truncate.ts`. Uses `unicode_width` for display-width
//! measurement instead of ink/stringWidth. Operates on `char` boundaries (Rust
//! strings are already valid UTF-8; for basic CJK/emoji support we rely on
//! `UnicodeWidthChar`).

use unicode_width::UnicodeWidthChar;

/// Returns the display width of a string in terminal columns.
///
/// This accounts for CJK characters (width 2) and zero-width characters.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| c.width().unwrap_or(0))
        .sum()
}

/// Truncates a string to fit within `max_width` terminal columns, appending
/// `...` (ellipsis char) when truncation occurs.
///
/// # Examples
/// ```
/// use crate::utils::truncate::truncate_to_width;
/// assert_eq!(truncate_to_width("hello world", 5), "hell\u{2026}");
/// assert_eq!(truncate_to_width("hi", 10), "hi");
/// ```
pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width <= 1 {
        return "\u{2026}".to_string(); // ...
    }
    let mut width = 0;
    let mut result = String::new();
    for c in text.chars() {
        let cw = c.width().unwrap_or(0);
        if width + cw > max_width - 1 {
            break;
        }
        result.push(c);
        width += cw;
    }
    result.push('\u{2026}');
    result
}

/// Truncates from the start of a string, keeping the tail end.
/// Prepends an ellipsis when truncation occurs.
///
/// # Examples
/// ```
/// use crate::utils::truncate::truncate_start_to_width;
/// assert_eq!(truncate_start_to_width("hello world", 6), "\u{2026}world");
/// assert_eq!(truncate_start_to_width("hi", 10), "hi");
/// ```
pub fn truncate_start_to_width(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width <= 1 {
        return "\u{2026}".to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut width = 0;
    let mut start_idx = chars.len();
    for i in (0..chars.len()).rev() {
        let cw = chars[i].width().unwrap_or(0);
        if width + cw > max_width - 1 {
            break;
        }
        width += cw;
        start_idx = i;
    }
    let tail: String = chars[start_idx..].iter().collect();
    format!("\u{2026}{}", tail)
}

/// Truncates a string to fit within `max_width` without appending an ellipsis.
///
/// Useful when the caller adds its own separator (e.g. middle-truncation).
pub fn truncate_to_width_no_ellipsis(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let mut width = 0;
    let mut result = String::new();
    for c in text.chars() {
        let cw = c.width().unwrap_or(0);
        if width + cw > max_width {
            break;
        }
        result.push(c);
        width += cw;
    }
    result
}

/// Truncates a file path in the middle to preserve both directory context and filename.
///
/// For example: `"src/components/deeply/nested/folder/MyComponent.tsx"` becomes
/// `"src/comp\u{2026}/MyComponent.tsx"` when `max_width` is 30.
///
/// # Examples
/// ```
/// use crate::utils::truncate::truncate_path_middle;
/// let short = truncate_path_middle("src/a/b/c/d/MyFile.rs", 20);
/// assert!(short.len() <= 25); // display width <= 20
/// assert!(short.contains("MyFile.rs"));
/// ```
pub fn truncate_path_middle(path: &str, max_width: usize) -> String {
    if display_width(path) <= max_width {
        return path.to_string();
    }
    if max_width == 0 {
        return "\u{2026}".to_string();
    }
    if max_width < 5 {
        return truncate_to_width(path, max_width);
    }

    let last_slash = path.rfind('/');
    let (directory, filename) = match last_slash {
        Some(idx) => (&path[..idx], &path[idx..]), // filename includes leading /
        None => ("", path),
    };
    let filename_width = display_width(filename);

    if filename_width >= max_width - 1 {
        return truncate_start_to_width(path, max_width);
    }

    // Result format: directory + "\u{2026}" + filename
    let available_for_dir = max_width.saturating_sub(1 + filename_width);
    if available_for_dir == 0 {
        return truncate_start_to_width(filename, max_width);
    }

    let truncated_dir = truncate_to_width_no_ellipsis(directory, available_for_dir);
    format!("{}\u{2026}{}", truncated_dir, filename)
}

/// Truncates a string to `max_width`, optionally collapsing to a single line first.
///
/// If `single_line` is true, truncates at the first newline and appends an ellipsis.
pub fn truncate(s: &str, max_width: usize, single_line: bool) -> String {
    let mut result = s.to_string();

    if single_line {
        if let Some(nl) = s.find('\n') {
            result = s[..nl].to_string();
            if display_width(&result) + 1 > max_width {
                return truncate_to_width(&result, max_width);
            }
            return format!("{}\u{2026}", result);
        }
    }

    if display_width(&result) <= max_width {
        return result;
    }
    truncate_to_width(&result, max_width)
}

/// Wraps text into lines that fit within `width` terminal columns.
///
/// Splits on character boundaries (not word boundaries) for simplicity.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for c in text.chars() {
        let cw = c.width().unwrap_or(0);
        if current_width + cw <= width {
            current_line.push(c);
            current_width += cw;
        } else {
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            current_line = c.to_string();
            current_width = cw;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn test_truncate_to_width_no_truncation() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_to_width_basic() {
        let result = truncate_to_width("hello world", 6);
        assert_eq!(result, "hello\u{2026}");
    }

    #[test]
    fn test_truncate_to_width_tiny() {
        assert_eq!(truncate_to_width("hello", 1), "\u{2026}");
        assert_eq!(truncate_to_width("hello", 0), "\u{2026}");
    }

    #[test]
    fn test_truncate_start_to_width() {
        let result = truncate_start_to_width("hello world", 6);
        assert_eq!(result, "\u{2026}world");
    }

    #[test]
    fn test_truncate_start_no_truncation() {
        assert_eq!(truncate_start_to_width("hi", 10), "hi");
    }

    #[test]
    fn test_truncate_path_middle_no_truncation() {
        assert_eq!(truncate_path_middle("src/main.rs", 30), "src/main.rs");
    }

    #[test]
    fn test_truncate_path_middle_truncates() {
        let result = truncate_path_middle("src/deeply/nested/folder/component/MyFile.rs", 25);
        assert!(result.contains("MyFile.rs"));
        assert!(result.contains('\u{2026}'));
    }

    #[test]
    fn test_truncate_single_line() {
        assert_eq!(truncate("hello\nworld", 20, true), "hello\u{2026}");
        assert_eq!(truncate("hello", 20, true), "hello");
        assert_eq!(truncate("hello", 20, false), "hello");
    }

    #[test]
    fn test_wrap_text() {
        let lines = wrap_text("hello world", 5);
        assert_eq!(lines, vec!["hello", " worl", "d"]);
    }

    #[test]
    fn test_wrap_text_fits() {
        let lines = wrap_text("hi", 10);
        assert_eq!(lines, vec!["hi"]);
    }
}
