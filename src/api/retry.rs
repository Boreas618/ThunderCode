//! Retry logic with exponential backoff and jitter.
//!
//! Ported from ref/services/api/withRetry.ts`.

use std::future::Future;
use std::time::Duration;

use crate::api::errors::{should_retry, ApiError};

// ---------------------------------------------------------------------------
// RetryConfig
// ---------------------------------------------------------------------------

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial attempt).
    pub max_retries: u32,
    /// Delay before the first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each attempt.
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(32),
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    /// Compute the delay for a given attempt (1-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.initial_delay.as_secs_f64()
            * self.backoff_factor.powi((attempt.saturating_sub(1)) as i32);
        let clamped = base.min(self.max_delay.as_secs_f64());
        // Add ~12.5% jitter (half of 25%) to spread out retries.
        let jitter = clamped * 0.125 * pseudo_random_fraction(attempt);
        Duration::from_secs_f64(clamped + jitter)
    }

    /// Compute the delay for a given attempt, optionally overridden by a
    /// `Retry-After` header value (in seconds).
    pub fn delay_for_attempt_with_header(
        &self,
        attempt: u32,
        retry_after_secs: Option<f64>,
    ) -> Duration {
        if let Some(secs) = retry_after_secs {
            // Honor the server's Retry-After header, but cap at max_delay.
            let capped = secs.min(self.max_delay.as_secs_f64());
            Duration::from_secs_f64(capped)
        } else {
            self.delay_for_attempt(attempt)
        }
    }
}

// ---------------------------------------------------------------------------
// with_retry
// ---------------------------------------------------------------------------

/// Execute an async operation with retry logic.
///
/// The closure `f` is called repeatedly until it succeeds or the maximum
/// number of retries is exhausted. Only errors that pass `should_retry` are
/// retried; other errors are returned immediately.
///
/// Returns the result of the first successful invocation, or the last error.
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, f: F) -> Result<T, ApiError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    let mut last_error: Option<ApiError> = None;

    for attempt in 0..=config.max_retries {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                // Don't retry errors that are not retryable.
                if !should_retry(&err) {
                    return Err(err);
                }

                tracing::debug!(
                    attempt = attempt + 1,
                    max = config.max_retries + 1,
                    "Retryable API error: {err}"
                );

                last_error = Some(err);

                // Don't sleep after the last attempt.
                if attempt < config.max_retries {
                    let delay = delay_for_error(config, attempt + 1, last_error.as_ref());
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ApiError::InvalidRequest {
        message: "Retry exhausted with no error".to_owned(),
    }))
}

/// Compute the delay, honoring `Retry-After` from rate-limit errors.
fn delay_for_error(config: &RetryConfig, attempt: u32, error: Option<&ApiError>) -> Duration {
    let retry_after_secs = error.and_then(|e| match e {
        ApiError::RateLimit { retry_after, .. } => {
            retry_after.map(|d| d.as_secs_f64())
        }
        _ => None,
    });
    config.delay_for_attempt_with_header(attempt, retry_after_secs)
}

/// Cheap deterministic-ish fraction in [0, 1) for jitter, seeded by attempt.
fn pseudo_random_fraction(attempt: u32) -> f64 {
    // Simple hash-like mixing for reproducible-but-varied jitter.
    let x = attempt.wrapping_mul(2654435761);
    (x as f64) / (u32::MAX as f64)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn default_config() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 10);
        assert_eq!(cfg.initial_delay, Duration::from_millis(500));
        assert_eq!(cfg.max_delay, Duration::from_secs(32));
    }

    #[test]
    fn delay_increases_exponentially() {
        let cfg = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
        };
        let d1 = cfg.delay_for_attempt(1);
        let d2 = cfg.delay_for_attempt(2);
        let d3 = cfg.delay_for_attempt(3);

        // Each delay should be roughly double the previous (plus jitter).
        assert!(d2 > d1, "d2={d2:?} should be > d1={d1:?}");
        assert!(d3 > d2, "d3={d3:?} should be > d2={d2:?}");
    }

    #[test]
    fn delay_capped_at_max() {
        let cfg = RetryConfig {
            max_retries: 20,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
        };
        let d = cfg.delay_for_attempt(20);
        // Should be at most max_delay + jitter (max_delay * 1.125).
        assert!(d <= Duration::from_secs_f64(10.0 * 1.15));
    }

    #[test]
    fn retry_after_header_override() {
        let cfg = RetryConfig::default();
        let d = cfg.delay_for_attempt_with_header(1, Some(5.0));
        assert_eq!(d, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let cfg = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 2.0,
        };

        let result = with_retry(&cfg, || {
            let attempts = attempts_clone.clone();
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(ApiError::Overloaded {
                        message: "try again".to_owned(),
                    })
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_fails_on_non_retryable() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let cfg = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 2.0,
        };

        let result: Result<i32, _> = with_retry(&cfg, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(ApiError::Authentication {
                    message: "bad key".to_owned(),
                })
            }
        })
        .await;

        assert!(result.is_err());
        // Non-retryable error should not retry.
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_exhausts_all_attempts() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let cfg = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 2.0,
        };

        let result: Result<i32, _> = with_retry(&cfg, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(ApiError::ServerError {
                    message: "oops".to_owned(),
                    status: 500,
                })
            }
        })
        .await;

        assert!(result.is_err());
        // 1 initial + 2 retries = 3 total.
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}
