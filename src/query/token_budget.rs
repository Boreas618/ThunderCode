//! Token budget tracking with continuation logic.
//!
//! Ported from ref/query/tokenBudget.ts`. Tracks whether the model should
//! continue generating when it hits `end_turn` but hasn't exhausted its
//! output budget, plus diminishing-returns detection to avoid wasting tokens
//! on repetitive low-value continuations.

use std::time::Instant;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Fraction of the budget that must be consumed before we stop continuing.
const COMPLETION_THRESHOLD: f64 = 0.9;

/// Minimum token delta between two successive checks. If the model produced
/// fewer tokens than this on two consecutive checks after at least 3
/// continuations, we declare diminishing returns and stop.
const DIMINISHING_THRESHOLD: u64 = 500;

// ---------------------------------------------------------------------------
// BudgetTracker
// ---------------------------------------------------------------------------

/// Tracks token budget across continuations within a single turn.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    /// How many times we have continued so far.
    pub continuation_count: u32,

    /// Token delta between the two most recent checks.
    last_delta_tokens: u64,

    /// Global turn token count at the last check.
    last_global_turn_tokens: u64,

    /// When this tracker was created.
    started_at: Instant,
}

impl BudgetTracker {
    /// Create a fresh tracker.
    pub fn new() -> Self {
        Self {
            continuation_count: 0,
            last_delta_tokens: 0,
            last_global_turn_tokens: 0,
            started_at: Instant::now(),
        }
    }

    /// Decide whether to continue generating.
    ///
    /// # Arguments
    /// * `budget` -- total output token budget for this turn. If `None` or
    ///   zero, always returns `Stop` (budget feature disabled or not
    ///   applicable).
    /// * `global_turn_tokens` -- cumulative output tokens used in this turn
    ///   so far (across all continuations).
    pub fn check(&mut self, budget: Option<u64>, global_turn_tokens: u64) -> BudgetDecision {
        let budget = match budget {
            Some(b) if b > 0 => b,
            _ => return BudgetDecision::Stop { event: None },
        };

        let turn_tokens = global_turn_tokens;
        let pct = ((turn_tokens as f64 / budget as f64) * 100.0).round() as u32;
        let delta_since_last = global_turn_tokens.saturating_sub(self.last_global_turn_tokens);

        // Diminishing returns: after 3+ continuations, if the last two deltas
        // are both below the threshold, stop early.
        let is_diminishing = self.continuation_count >= 3
            && delta_since_last < DIMINISHING_THRESHOLD
            && self.last_delta_tokens < DIMINISHING_THRESHOLD;

        // Keep going if not diminishing and under the completion threshold.
        if !is_diminishing && (turn_tokens as f64) < (budget as f64 * COMPLETION_THRESHOLD) {
            self.continuation_count += 1;
            self.last_delta_tokens = delta_since_last;
            self.last_global_turn_tokens = global_turn_tokens;

            return BudgetDecision::Continue {
                nudge_message: format!(
                    "Token budget: {}% used ({} / {}). Continue.",
                    pct, turn_tokens, budget
                ),
                continuation_count: self.continuation_count,
                pct,
                turn_tokens,
                budget,
            };
        }

        // Report a completion event if we ever continued at all.
        if is_diminishing || self.continuation_count > 0 {
            return BudgetDecision::Stop {
                event: Some(BudgetCompletionEvent {
                    continuation_count: self.continuation_count,
                    pct,
                    turn_tokens,
                    budget,
                    diminishing_returns: is_diminishing,
                    duration_ms: self.started_at.elapsed().as_millis() as u64,
                }),
            };
        }

        BudgetDecision::Stop { event: None }
    }
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Decision types
// ---------------------------------------------------------------------------

/// Decision from `BudgetTracker::check`.
#[derive(Debug, Clone)]
pub enum BudgetDecision {
    /// Keep generating -- inject the `nudge_message` as a meta user message.
    Continue {
        nudge_message: String,
        continuation_count: u32,
        pct: u32,
        turn_tokens: u64,
        budget: u64,
    },

    /// Stop generating. `event` is `Some` if there is a meaningful
    /// completion event to log (i.e. we continued at least once).
    Stop {
        event: Option<BudgetCompletionEvent>,
    },
}

/// Analytics payload emitted when a budget-tracked turn completes.
#[derive(Debug, Clone)]
pub struct BudgetCompletionEvent {
    pub continuation_count: u32,
    pub pct: u32,
    pub turn_tokens: u64,
    pub budget: u64,
    pub diminishing_returns: bool,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_when_no_budget() {
        let mut tracker = BudgetTracker::new();
        let decision = tracker.check(None, 1000);
        assert!(matches!(decision, BudgetDecision::Stop { event: None }));
    }

    #[test]
    fn stops_when_zero_budget() {
        let mut tracker = BudgetTracker::new();
        let decision = tracker.check(Some(0), 1000);
        assert!(matches!(decision, BudgetDecision::Stop { event: None }));
    }

    #[test]
    fn continues_when_under_threshold() {
        let mut tracker = BudgetTracker::new();
        // 1000 out of 10000 = 10%, well under 90%
        let decision = tracker.check(Some(10000), 1000);
        match decision {
            BudgetDecision::Continue {
                continuation_count,
                pct,
                ..
            } => {
                assert_eq!(continuation_count, 1);
                assert_eq!(pct, 10);
            }
            other => panic!("expected Continue, got: {other:?}"),
        }
    }

    #[test]
    fn stops_when_over_threshold() {
        let mut tracker = BudgetTracker::new();
        // 9500 out of 10000 = 95%, over 90%
        let decision = tracker.check(Some(10000), 9500);
        assert!(matches!(decision, BudgetDecision::Stop { event: None }));
    }

    #[test]
    fn detects_diminishing_returns() {
        let mut tracker = BudgetTracker::new();
        // Simulate 3 continuations with small deltas
        tracker.continuation_count = 3;
        tracker.last_delta_tokens = 100; // below 500
        tracker.last_global_turn_tokens = 5000;

        // Delta = 5100 - 5000 = 100, also below 500
        let decision = tracker.check(Some(10000), 5100);
        match decision {
            BudgetDecision::Stop { event: Some(evt) } => {
                assert!(evt.diminishing_returns);
            }
            other => panic!("expected Stop with diminishing returns, got: {other:?}"),
        }
    }

    #[test]
    fn continues_multiple_times() {
        let mut tracker = BudgetTracker::new();

        // First continuation
        let d1 = tracker.check(Some(10000), 1000);
        assert!(matches!(d1, BudgetDecision::Continue { .. }));

        // Second continuation
        let d2 = tracker.check(Some(10000), 3000);
        assert!(matches!(d2, BudgetDecision::Continue { .. }));

        // Third -- still under 90%
        let d3 = tracker.check(Some(10000), 5000);
        assert!(matches!(d3, BudgetDecision::Continue { .. }));
        assert_eq!(tracker.continuation_count, 3);
    }
}
