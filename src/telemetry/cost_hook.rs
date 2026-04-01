//! Cost display formatting for session exit summaries.
//!
//! Ported from ref/costHook.ts and the `formatTotalCost` function in
//! ref/cost-tracker.ts.  Produces human-readable cost and usage strings
//! suitable for printing to the terminal on session exit.

use std::fmt::Write;

use crate::telemetry::cost_tracker::CostSummary;

// ---------------------------------------------------------------------------
// CodeChanges
// ---------------------------------------------------------------------------

/// Tracks lines added/removed during a session, shown in the exit summary.
#[derive(Debug, Clone, Default)]
pub struct CodeChanges {
    pub lines_added: u64,
    pub lines_removed: u64,
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format a USD cost for display.
///
/// Costs above $0.50 are shown with 2 decimal places (e.g. `$1.23`).
/// Smaller costs show up to `max_decimals` places (default 4) so the user
/// can see sub-cent amounts during light usage.
fn format_cost(cost: f64, max_decimals: usize) -> String {
    if cost > 0.5 {
        // Round to nearest cent
        let rounded = (cost * 100.0).round() / 100.0;
        format!("${:.2}", rounded)
    } else {
        format!("${:.*}", max_decimals, cost)
    }
}

/// Format a token count in compact notation (e.g. `1.3k`, `2.1m`).
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        let v = n as f64 / 1_000_000.0;
        format!("{:.1}m", v)
    } else if n >= 1_000 {
        let v = n as f64 / 1_000.0;
        format!("{:.1}k", v)
    } else {
        n.to_string()
    }
}

/// Format a duration in milliseconds to a human-readable string.
///
/// Follows the ref/utils/format.ts `formatDuration` logic:
/// - < 60s  -> `"Xs"`
/// - >= 60s -> `"Xm Ys"`, `"Xh Ym Zs"`, `"Xd Yh Zm"`
fn format_duration(ms: u64) -> String {
    if ms == 0 {
        return "0s".to_string();
    }
    if ms < 60_000 {
        let s = ms / 1000;
        return format!("{}s", s);
    }

    let total_secs = (ms as f64 / 1000.0).round() as u64;
    let mut days = total_secs / 86400;
    let mut hours = (total_secs % 86400) / 3600;
    let mut minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    // The reference implementation handles carry-over from rounding, but
    // since we pre-round total_secs that cannot happen here. Keep the
    // cascade for safety.
    if minutes == 60 {
        minutes = 0;
        hours += 1;
    }
    if hours == 24 {
        hours = 0;
        days += 1;
    }

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else {
        format!("{}m {}s", minutes, seconds)
    }
}

/// Pluralise "line" / "lines".
fn lines_word(n: u64) -> &'static str {
    if n == 1 {
        "line"
    } else {
        "lines"
    }
}

// ---------------------------------------------------------------------------
// Public formatting functions
// ---------------------------------------------------------------------------

/// Format a `CostSummary` as a multi-line usage string (no code-change info).
///
/// Mirrors the body of `formatTotalCost()` from ref/cost-tracker.ts.
pub fn format_cost_summary(summary: &CostSummary) -> String {
    let mut out = String::new();

    let cost_display = format_cost(summary.total_cost_usd, 4);

    let _ = writeln!(out, "Total cost:            {}", cost_display);
    let _ = writeln!(
        out,
        "Total duration (API):  {}",
        format_duration(summary.total_duration_ms)
    );

    // Per-model breakdown
    if summary.model_breakdown.is_empty() {
        let _ = write!(
            out,
            "Usage:                 0 input, 0 output, 0 cache read, 0 cache write"
        );
    } else {
        let _ = write!(out, "Usage by model:");

        // Sort models for deterministic output
        let mut models: Vec<(&String, &crate::telemetry::cost_tracker::ModelCosts)> =
            summary.model_breakdown.iter().collect();
        models.sort_by_key(|(name, _)| name.to_string());

        for (model, usage) in models {
            let _ = write!(
                out,
                "\n{:>21}  {} input, {} output, {} cache read, {} cache write ({})",
                format!("{}:", model),
                format_number(usage.input_tokens),
                format_number(usage.output_tokens),
                format_number(usage.cache_read_tokens),
                format_number(usage.cache_write_tokens),
                format_cost(usage.cost_usd, 4),
            );
        }
    }

    out
}

/// Format a full exit summary including code changes.
///
/// Mirrors `formatTotalCost()` from ref/cost-tracker.ts with the
/// `Total code changes` line added.
pub fn format_exit_summary(summary: &CostSummary, code_changes: &CodeChanges) -> String {
    let mut out = String::new();

    let cost_display = format_cost(summary.total_cost_usd, 4);

    let _ = writeln!(out, "Total cost:            {}", cost_display);
    let _ = writeln!(
        out,
        "Total duration (API):  {}",
        format_duration(summary.total_duration_ms)
    );
    let _ = writeln!(
        out,
        "Total code changes:    {} {} added, {} {} removed",
        code_changes.lines_added,
        lines_word(code_changes.lines_added),
        code_changes.lines_removed,
        lines_word(code_changes.lines_removed),
    );

    // Per-model breakdown
    if summary.model_breakdown.is_empty() {
        let _ = write!(
            out,
            "Usage:                 0 input, 0 output, 0 cache read, 0 cache write"
        );
    } else {
        let _ = write!(out, "Usage by model:");

        let mut models: Vec<(&String, &crate::telemetry::cost_tracker::ModelCosts)> =
            summary.model_breakdown.iter().collect();
        models.sort_by_key(|(name, _)| name.to_string());

        for (model, usage) in models {
            let _ = write!(
                out,
                "\n{:>21}  {} input, {} output, {} cache read, {} cache write ({})",
                format!("{}:", model),
                format_number(usage.input_tokens),
                format_number(usage.output_tokens),
                format_number(usage.cache_read_tokens),
                format_number(usage.cache_write_tokens),
                format_cost(usage.cost_usd, 4),
            );
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::cost_tracker::{CostSummary, ModelCosts};
    use std::collections::HashMap;

    fn sample_summary() -> CostSummary {
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "primary-sonnet-4".to_string(),
            ModelCosts {
                input_tokens: 150_000,
                output_tokens: 30_000,
                cache_read_tokens: 100_000,
                cache_write_tokens: 5_000,
                cost_usd: 0.0315,
                call_count: 5,
            },
        );
        CostSummary {
            total_cost_usd: 0.0315,
            total_duration_ms: 45_000,
            total_input_tokens: 150_000,
            total_output_tokens: 30_000,
            total_cache_read_tokens: 100_000,
            total_cache_write_tokens: 5_000,
            model_breakdown: breakdown,
        }
    }

    #[test]
    fn test_format_cost_small() {
        assert_eq!(format_cost(0.0012, 4), "$0.0012");
    }

    #[test]
    fn test_format_cost_large() {
        assert_eq!(format_cost(1.567, 4), "$1.57");
    }

    #[test]
    fn test_format_number_small() {
        assert_eq!(format_number(42), "42");
    }

    #[test]
    fn test_format_number_thousands() {
        assert_eq!(format_number(1_500), "1.5k");
    }

    #[test]
    fn test_format_number_millions() {
        assert_eq!(format_number(2_100_000), "2.1m");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(5_000), "5s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(125_000), "2m 5s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3_661_000), "1h 1m 1s");
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn test_format_cost_summary_contains_model() {
        let s = format_cost_summary(&sample_summary());
        assert!(s.contains("primary-sonnet-4:"));
        assert!(s.contains("150.0k input"));
        assert!(s.contains("30.0k output"));
    }

    #[test]
    fn test_format_exit_summary_contains_code_changes() {
        let changes = CodeChanges {
            lines_added: 42,
            lines_removed: 7,
        };
        let s = format_exit_summary(&sample_summary(), &changes);
        assert!(s.contains("42 lines added"));
        assert!(s.contains("7 lines removed"));
    }

    #[test]
    fn test_format_exit_summary_singular_line() {
        let changes = CodeChanges {
            lines_added: 1,
            lines_removed: 1,
        };
        let s = format_exit_summary(&sample_summary(), &changes);
        assert!(s.contains("1 line added"));
        assert!(s.contains("1 line removed"));
    }

    #[test]
    fn test_format_cost_summary_empty_breakdown() {
        let summary = CostSummary {
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_read_tokens: 0,
            total_cache_write_tokens: 0,
            model_breakdown: HashMap::new(),
        };
        let s = format_cost_summary(&summary);
        assert!(s.contains("Usage:"));
        assert!(s.contains("0 input"));
    }
}
