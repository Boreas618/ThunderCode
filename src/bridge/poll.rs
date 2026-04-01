//! Poll interval configuration and defaults.
//!
//! Ported from ref/bridge/pollConfigDefaults.ts` and `ref/bridge/pollConfig.ts`.

use std::time::Duration;

/// Poll interval configuration for the bridge loop.
#[derive(Debug, Clone)]
pub struct PollIntervalConfig {
    /// Poll interval when actively seeking work (below max sessions).
    pub poll_interval_not_at_capacity: Duration,
    /// Poll interval when at session capacity.
    pub poll_interval_at_capacity: Duration,
    /// Heartbeat interval when at capacity (0 = disabled).
    pub heartbeat_interval: Duration,
    /// Multisession: poll interval when not at capacity.
    pub multisession_poll_not_at_capacity: Duration,
    /// Multisession: poll interval when partially loaded.
    pub multisession_poll_partial_capacity: Duration,
    /// Multisession: poll interval when at capacity.
    pub multisession_poll_at_capacity: Duration,
    /// Reclaim threshold for stale work items.
    pub reclaim_older_than: Duration,
    /// Session keepalive interval.
    pub session_keepalive_interval: Duration,
}

impl Default for PollIntervalConfig {
    fn default() -> Self {
        DEFAULT_POLL_CONFIG
    }
}

/// Default poll interval configuration.
///
/// - 2s when seeking work (not at capacity).
/// - 10min when at capacity (liveness signal).
/// - 5s reclaim threshold for stale work.
/// - 2min session keepalive.
pub const DEFAULT_POLL_CONFIG: PollIntervalConfig = PollIntervalConfig {
    poll_interval_not_at_capacity: Duration::from_millis(2000),
    poll_interval_at_capacity: Duration::from_millis(600_000), // 10 minutes
    heartbeat_interval: Duration::ZERO, // disabled by default
    multisession_poll_not_at_capacity: Duration::from_millis(2000),
    multisession_poll_partial_capacity: Duration::from_millis(2000),
    multisession_poll_at_capacity: Duration::from_millis(600_000),
    reclaim_older_than: Duration::from_millis(5000),
    session_keepalive_interval: Duration::from_millis(120_000),
};

impl PollIntervalConfig {
    /// Returns the appropriate poll interval based on current capacity.
    ///
    /// - `active_sessions` - Number of currently active sessions.
    /// - `max_sessions` - Maximum number of concurrent sessions.
    pub fn poll_interval(&self, active_sessions: usize, max_sessions: usize) -> Duration {
        if max_sessions <= 1 {
            // Single-session mode.
            if active_sessions >= max_sessions {
                self.poll_interval_at_capacity
            } else {
                self.poll_interval_not_at_capacity
            }
        } else {
            // Multi-session mode.
            if active_sessions >= max_sessions {
                self.multisession_poll_at_capacity
            } else if active_sessions > 0 {
                self.multisession_poll_partial_capacity
            } else {
                self.multisession_poll_not_at_capacity
            }
        }
    }

    /// Whether heartbeat is enabled.
    pub fn heartbeat_enabled(&self) -> bool {
        !self.heartbeat_interval.is_zero()
    }

    /// Returns the reclaim threshold in milliseconds for the poll query param.
    pub fn reclaim_older_than_ms(&self) -> u64 {
        self.reclaim_older_than.as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PollIntervalConfig::default();
        assert_eq!(config.poll_interval_not_at_capacity, Duration::from_millis(2000));
        assert_eq!(config.poll_interval_at_capacity, Duration::from_millis(600_000));
        assert!(!config.heartbeat_enabled());
    }

    #[test]
    fn test_poll_interval_single_session() {
        let config = PollIntervalConfig::default();
        // Not at capacity.
        assert_eq!(
            config.poll_interval(0, 1),
            Duration::from_millis(2000)
        );
        // At capacity.
        assert_eq!(
            config.poll_interval(1, 1),
            Duration::from_millis(600_000)
        );
    }

    #[test]
    fn test_poll_interval_multi_session() {
        let config = PollIntervalConfig::default();
        // Empty.
        assert_eq!(
            config.poll_interval(0, 4),
            Duration::from_millis(2000)
        );
        // Partial.
        assert_eq!(
            config.poll_interval(2, 4),
            Duration::from_millis(2000)
        );
        // At capacity.
        assert_eq!(
            config.poll_interval(4, 4),
            Duration::from_millis(600_000)
        );
    }

    #[test]
    fn test_reclaim_older_than_ms() {
        let config = PollIntervalConfig::default();
        assert_eq!(config.reclaim_older_than_ms(), 5000);
    }
}
