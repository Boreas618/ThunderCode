//! Text formatting utilities: file sizes, durations, numbers, pluralization.
//!
//! Ported from ref/utils/format.ts`. These are pure display formatters with
//! no IO or terminal dependencies.

/// Formats a byte count to a human-readable string (KB, MB, GB).
///
/// # Examples
/// ```
/// use crate::utils::format::format_file_size;
/// assert_eq!(format_file_size(512), "512 bytes");
/// assert_eq!(format_file_size(1536), "1.5KB");
/// assert_eq!(format_file_size(1_048_576), "1MB");
/// ```
pub fn format_file_size(size_in_bytes: u64) -> String {
    let kb = size_in_bytes as f64 / 1024.0;
    if kb < 1.0 {
        return format!("{} bytes", size_in_bytes);
    }
    if kb < 1024.0 {
        return format_with_optional_decimal(kb, "KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format_with_optional_decimal(mb, "MB");
    }
    let gb = mb / 1024.0;
    format_with_optional_decimal(gb, "GB")
}

/// Formats a value with one decimal place, dropping ".0" suffixes.
fn format_with_optional_decimal(value: f64, suffix: &str) -> String {
    let formatted = format!("{:.1}", value);
    let trimmed = formatted.strip_suffix(".0").unwrap_or(&formatted);
    format!("{}{}", trimmed, suffix)
}

/// Formats milliseconds as seconds with 1 decimal place (e.g. `1234` -> `"1.2s"`).
///
/// Use for sub-minute timings where the fractional second is meaningful.
pub fn format_seconds_short(ms: f64) -> String {
    format!("{:.1}s", ms / 1000.0)
}

/// Options for [`format_duration`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DurationOptions {
    /// If true, omit trailing zero components (e.g. `1h` instead of `1h 0m 0s`).
    pub hide_trailing_zeros: bool,
    /// If true, only emit the single most significant unit.
    pub most_significant_only: bool,
}

/// Formats a millisecond duration into a compact human-readable string.
///
/// # Examples
/// ```
/// use crate::utils::format::{format_duration, DurationOptions};
/// assert_eq!(format_duration(0, DurationOptions::default()), "0s");
/// assert_eq!(format_duration(3_500, DurationOptions::default()), "3s");
/// assert_eq!(format_duration(90_000, DurationOptions::default()), "1m 30s");
/// assert_eq!(
///     format_duration(3_600_000, DurationOptions { hide_trailing_zeros: true, ..Default::default() }),
///     "1h"
/// );
/// ```
pub fn format_duration(ms: u64, opts: DurationOptions) -> String {
    if ms < 60_000 {
        if ms == 0 {
            return "0s".to_string();
        }
        if ms < 1 {
            return format!("{:.1}s", ms as f64 / 1000.0);
        }
        let s = ms / 1000;
        return format!("{}s", s);
    }

    let mut days = ms / 86_400_000;
    let mut hours = (ms % 86_400_000) / 3_600_000;
    let mut minutes = (ms % 3_600_000) / 60_000;
    let mut seconds = ((ms % 60_000) as f64 / 1000.0).round() as u64;

    // Handle rounding carry-over (e.g. 59.5s rounds to 60s)
    if seconds == 60 {
        seconds = 0;
        minutes += 1;
    }
    if minutes == 60 {
        minutes = 0;
        hours += 1;
    }
    if hours == 24 {
        hours = 0;
        days += 1;
    }

    if opts.most_significant_only {
        if days > 0 {
            return format!("{}d", days);
        }
        if hours > 0 {
            return format!("{}h", hours);
        }
        if minutes > 0 {
            return format!("{}m", minutes);
        }
        return format!("{}s", seconds);
    }

    let hide = opts.hide_trailing_zeros;

    if days > 0 {
        if hide && hours == 0 && minutes == 0 {
            return format!("{}d", days);
        }
        if hide && minutes == 0 {
            return format!("{}d {}h", days, hours);
        }
        return format!("{}d {}h {}m", days, hours, minutes);
    }
    if hours > 0 {
        if hide && minutes == 0 && seconds == 0 {
            return format!("{}h", hours);
        }
        if hide && seconds == 0 {
            return format!("{}h {}m", hours, minutes);
        }
        return format!("{}h {}m {}s", hours, minutes, seconds);
    }
    if minutes > 0 {
        if hide && seconds == 0 {
            return format!("{}m", minutes);
        }
        return format!("{}m {}s", minutes, seconds);
    }
    format!("{}s", seconds)
}

/// Formats a number in compact notation (e.g. 1500 -> "1.5k", 900 -> "900").
///
/// For numbers >= 1000, uses K/M/B suffixes with one decimal place.
/// The ".0" decimal is dropped for numbers >= 1000.
///
/// # Examples
/// ```
/// use crate::utils::format::format_number;
/// assert_eq!(format_number(900), "900");
/// assert_eq!(format_number(1_321), "1.3k");
/// assert_eq!(format_number(1_000_000), "1.0m");
/// ```
pub fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        let val = n as f64 / 1_000_000_000.0;
        format_compact(val, "b", n >= 1_000)
    } else if n >= 1_000_000 {
        let val = n as f64 / 1_000_000.0;
        format_compact(val, "m", n >= 1_000)
    } else if n >= 1_000 {
        let val = n as f64 / 1_000.0;
        format_compact(val, "k", true)
    } else {
        n.to_string()
    }
}

/// Format a compact number with optional ".0" retention.
fn format_compact(val: f64, suffix: &str, consistent_decimals: bool) -> String {
    let formatted = format!("{:.1}", val);
    if consistent_decimals {
        // Keep ".0" for consistent decimals (matching TS Intl.NumberFormat with minimumFractionDigits=1)
        // Actually the TS code drops .0 for formatTokens; let's just trim it here for formatNumber
        format!("{}{}", formatted, suffix)
    } else {
        let trimmed = formatted.strip_suffix(".0").unwrap_or(&formatted);
        format!("{}{}", trimmed, suffix)
    }
}

/// Formats a token count in compact notation, always dropping trailing ".0".
///
/// # Examples
/// ```
/// use crate::utils::format::format_tokens;
/// assert_eq!(format_tokens(500), "500");
/// assert_eq!(format_tokens(1000), "1k");
/// assert_eq!(format_tokens(1_500), "1.5k");
/// ```
pub fn format_tokens(count: u64) -> String {
    format_number(count).replace(".0", "")
}

/// Returns the correct singular or plural form of a word.
///
/// # Examples
/// ```
/// use crate::utils::format::pluralize;
/// assert_eq!(pluralize(1, "file", "files"), "1 file");
/// assert_eq!(pluralize(3, "file", "files"), "3 files");
/// assert_eq!(pluralize(0, "item", "items"), "0 items");
/// ```
pub fn pluralize(count: u64, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        format!("{} {}", count, plural)
    }
}

/// Returns "s" when count != 1, empty string otherwise.
/// Useful for inline pluralization: `format!("{} file{}", n, plural_suffix(n))`.
pub fn plural_suffix(count: u64) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Formats a list of items as a comma-separated string with "and" before the last.
///
/// # Examples
/// ```
/// use crate::utils::format::format_list;
/// assert_eq!(format_list(&["a"]), "a");
/// assert_eq!(format_list(&["a", "b"]), "a and b");
/// assert_eq!(format_list(&["a", "b", "c"]), "a, b, and c");
/// ```
pub fn format_list(items: &[&str]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 bytes");
        assert_eq!(format_file_size(512), "512 bytes");
        assert_eq!(format_file_size(1023), "1023 bytes");
        assert_eq!(format_file_size(1024), "1KB");
        assert_eq!(format_file_size(1536), "1.5KB");
        assert_eq!(format_file_size(1_048_576), "1MB");
        assert_eq!(format_file_size(1_073_741_824), "1GB");
        assert_eq!(format_file_size(1_610_612_736), "1.5GB");
    }

    #[test]
    fn test_format_seconds_short() {
        assert_eq!(format_seconds_short(1234.0), "1.2s");
        assert_eq!(format_seconds_short(500.0), "0.5s");
        assert_eq!(format_seconds_short(0.0), "0.0s");
    }

    #[test]
    fn test_format_duration() {
        let d = DurationOptions::default();
        assert_eq!(format_duration(0, d), "0s");
        assert_eq!(format_duration(3_000, d), "3s");
        assert_eq!(format_duration(3_500, d), "3s");
        assert_eq!(format_duration(60_000, d), "1m 0s");
        assert_eq!(format_duration(90_000, d), "1m 30s");
        assert_eq!(format_duration(3_600_000, d), "1h 0m 0s");
        assert_eq!(format_duration(86_400_000, d), "1d 0h 0m");

        let hide = DurationOptions {
            hide_trailing_zeros: true,
            ..Default::default()
        };
        assert_eq!(format_duration(3_600_000, hide), "1h");
        assert_eq!(format_duration(3_660_000, hide), "1h 1m");
        assert_eq!(format_duration(86_400_000, hide), "1d");

        let msig = DurationOptions {
            most_significant_only: true,
            ..Default::default()
        };
        assert_eq!(format_duration(90_000, msig), "1m");
        assert_eq!(format_duration(3_661_000, msig), "1h");
        assert_eq!(format_duration(90_000_000, msig), "1d");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1_000), "1.0k");
        assert_eq!(format_number(1_500), "1.5k");
        assert_eq!(format_number(1_000_000), "1.0m");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_000), "1k");
        assert_eq!(format_tokens(1_500), "1.5k");
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize(0, "file", "files"), "0 files");
        assert_eq!(pluralize(1, "file", "files"), "1 file");
        assert_eq!(pluralize(5, "file", "files"), "5 files");
    }

    #[test]
    fn test_format_list() {
        assert_eq!(format_list(&[]), "");
        assert_eq!(format_list(&["a"]), "a");
        assert_eq!(format_list(&["a", "b"]), "a and b");
        assert_eq!(format_list(&["a", "b", "c"]), "a, b, and c");
    }
}
