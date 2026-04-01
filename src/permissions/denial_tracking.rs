//! Denial tracking state for permission classifiers.
//!
//! Ported from ref/utils/permissions/denialTracking.ts`.
//!
//! Tracks consecutive denials per tool and total denials per session.
//! When thresholds are exceeded, the system falls back to prompting
//! the user instead of auto-denying.

/// Configurable denial limits.
pub struct DenialLimits {
    /// Fall back to prompting after this many consecutive denials.
    pub max_consecutive: u32,
    /// Fall back to prompting after this many total denials in a session.
    pub max_total: u32,
}

/// Default limits matching the TypeScript reference.
pub const DEFAULT_DENIAL_LIMITS: DenialLimits = DenialLimits {
    max_consecutive: 3,
    max_total: 20,
};

/// Session-scoped denial tracking state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenialTracker {
    pub consecutive_denials: u32,
    pub total_denials: u32,
    pub limits: DenialLimitsSnapshot,
}

/// Snapshot of limits stored alongside the tracker so the values travel
/// together (avoids needing a separate config reference at check time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DenialLimitsSnapshot {
    pub max_consecutive: u32,
    pub max_total: u32,
}

impl Default for DenialLimitsSnapshot {
    fn default() -> Self {
        Self {
            max_consecutive: DEFAULT_DENIAL_LIMITS.max_consecutive,
            max_total: DEFAULT_DENIAL_LIMITS.max_total,
        }
    }
}

impl Default for DenialTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DenialTracker {
    /// Create a fresh tracker with default limits.
    pub fn new() -> Self {
        Self {
            consecutive_denials: 0,
            total_denials: 0,
            limits: DenialLimitsSnapshot::default(),
        }
    }

    /// Create a tracker with custom limits.
    pub fn with_limits(max_consecutive: u32, max_total: u32) -> Self {
        Self {
            consecutive_denials: 0,
            total_denials: 0,
            limits: DenialLimitsSnapshot {
                max_consecutive,
                max_total,
            },
        }
    }

    /// Record a denial. Returns the updated tracker (functional style).
    pub fn record_denial(&self) -> Self {
        Self {
            consecutive_denials: self.consecutive_denials + 1,
            total_denials: self.total_denials + 1,
            limits: self.limits,
        }
    }

    /// Record a successful (non-denied) tool use. Resets the consecutive
    /// denial counter. Returns the same tracker if already at zero.
    pub fn record_success(&self) -> Self {
        if self.consecutive_denials == 0 {
            return self.clone();
        }
        Self {
            consecutive_denials: 0,
            total_denials: self.total_denials,
            limits: self.limits,
        }
    }

    /// Returns `true` when the tracker has exceeded either the consecutive
    /// or total denial limit, indicating the system should fall back to
    /// prompting the user.
    pub fn should_fallback_to_prompting(&self) -> bool {
        self.consecutive_denials >= self.limits.max_consecutive
            || self.total_denials >= self.limits.max_total
    }

    /// Reset the total denials counter (used after hitting the total limit
    /// to allow the user to continue after reviewing).
    pub fn reset_totals(&self) -> Self {
        Self {
            consecutive_denials: 0,
            total_denials: 0,
            limits: self.limits,
        }
    }

    /// Record a denial in place (mutable style, for subagent local tracking).
    pub fn record_denial_mut(&mut self) {
        self.consecutive_denials += 1;
        self.total_denials += 1;
    }

    /// Record a success in place.
    pub fn record_success_mut(&mut self) {
        self.consecutive_denials = 0;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_tracker_is_not_in_fallback() {
        let t = DenialTracker::new();
        assert!(!t.should_fallback_to_prompting());
    }

    #[test]
    fn consecutive_limit_triggers_fallback() {
        let mut t = DenialTracker::new();
        for _ in 0..3 {
            t = t.record_denial();
        }
        assert!(t.should_fallback_to_prompting());
        assert_eq!(t.consecutive_denials, 3);
    }

    #[test]
    fn success_resets_consecutive() {
        let t = DenialTracker::new()
            .record_denial()
            .record_denial()
            .record_success();
        assert_eq!(t.consecutive_denials, 0);
        assert_eq!(t.total_denials, 2);
        assert!(!t.should_fallback_to_prompting());
    }

    #[test]
    fn total_limit_triggers_fallback() {
        let mut t = DenialTracker::new();
        for _ in 0..20 {
            t = t.record_denial();
            if t.consecutive_denials >= 3 {
                // Simulate the system resetting consecutive after prompting.
                t = DenialTracker {
                    consecutive_denials: 0,
                    total_denials: t.total_denials,
                    limits: t.limits,
                };
            }
        }
        assert!(t.should_fallback_to_prompting());
    }

    #[test]
    fn custom_limits() {
        let t = DenialTracker::with_limits(1, 5)
            .record_denial();
        assert!(t.should_fallback_to_prompting());
    }

    #[test]
    fn reset_totals() {
        let t = DenialTracker::new()
            .record_denial()
            .record_denial()
            .record_denial()
            .reset_totals();
        assert_eq!(t.consecutive_denials, 0);
        assert_eq!(t.total_denials, 0);
        assert!(!t.should_fallback_to_prompting());
    }

    #[test]
    fn mutable_tracking() {
        let mut t = DenialTracker::new();
        t.record_denial_mut();
        t.record_denial_mut();
        assert_eq!(t.consecutive_denials, 2);
        assert_eq!(t.total_denials, 2);
        t.record_success_mut();
        assert_eq!(t.consecutive_denials, 0);
        assert_eq!(t.total_denials, 2);
    }
}
