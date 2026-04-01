//! Async sleep wrapper with cancellation support.
//!
//! Ported from ref/utils/sleep.ts`. Uses `tokio` for the timer and
//! `tokio_util::sync::CancellationToken` patterns for abort signalling.

use std::time::Duration;
use tokio::time;

/// Sleep for `duration`, cancellable via a `CancellationToken`.
///
/// If `cancel` is provided and gets cancelled before the sleep completes:
/// - Returns `Ok(())` by default (the caller should check `cancel.is_cancelled()`).
/// - Returns `Err(Aborted)` if `throw_on_abort` is `true`.
///
/// # Examples
/// ```
/// # tokio_test::block_on(async {
/// use crate::utils::sleep::sleep;
/// use std::time::Duration;
///
/// sleep(Duration::from_millis(10), None).await.unwrap();
/// # });
/// ```
pub async fn sleep(
    duration: Duration,
    cancel: Option<&tokio::sync::watch::Receiver<bool>>,
) -> Result<(), SleepAborted> {
    sleep_inner(duration, cancel, false).await
}

/// Sleep that returns `Err(SleepAborted)` when cancelled.
pub async fn sleep_throw_on_abort(
    duration: Duration,
    cancel: Option<&tokio::sync::watch::Receiver<bool>>,
) -> Result<(), SleepAborted> {
    sleep_inner(duration, cancel, true).await
}

async fn sleep_inner(
    duration: Duration,
    cancel: Option<&tokio::sync::watch::Receiver<bool>>,
    throw_on_abort: bool,
) -> Result<(), SleepAborted> {
    match cancel {
        None => {
            time::sleep(duration).await;
            Ok(())
        }
        Some(rx) => {
            // Check if already cancelled
            if *rx.borrow() {
                return if throw_on_abort {
                    Err(SleepAborted)
                } else {
                    Ok(())
                };
            }

            let mut rx = rx.clone();
            tokio::select! {
                _ = time::sleep(duration) => Ok(()),
                _ = rx.changed() => {
                    if throw_on_abort {
                        Err(SleepAborted)
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }
}

/// Error returned when a sleep is aborted via cancellation.
#[derive(Debug, Clone, thiserror::Error)]
#[error("sleep aborted")]
pub struct SleepAborted;

/// Race a future against a timeout. Returns `Err(TimeoutError)` if the future
/// doesn't complete within `duration`.
///
/// Equivalent to the TS `withTimeout(promise, ms, message)` helper.
///
/// # Examples
/// ```
/// use crate::utils::sleep::with_timeout;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let result = with_timeout(async { 42 }, Duration::from_secs(1), "test").await;
/// assert_eq!(result.unwrap(), 42);
/// # });
/// ```
pub async fn with_timeout<T>(
    future: impl std::future::Future<Output = T>,
    duration: Duration,
    message: &str,
) -> Result<T, TimeoutError> {
    match time::timeout(duration, future).await {
        Ok(v) => Ok(v),
        Err(_) => Err(TimeoutError(message.to_string())),
    }
}

/// Error returned when an operation times out.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct TimeoutError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sleep_completes() {
        let result = sleep(Duration::from_millis(10), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sleep_abort_silent() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        // Cancel immediately
        tx.send(true).unwrap();
        let result = sleep(Duration::from_secs(10), Some(&rx)).await;
        assert!(result.is_ok()); // silent abort
    }

    #[tokio::test]
    async fn test_sleep_abort_throws() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        tx.send(true).unwrap();
        let result = sleep_throw_on_abort(Duration::from_secs(10), Some(&rx)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_with_timeout_success() {
        let result = with_timeout(async { 42 }, Duration::from_secs(1), "test").await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_timeout_expires() {
        let result = with_timeout(
            async {
                time::sleep(Duration::from_secs(10)).await;
                42
            },
            Duration::from_millis(10),
            "took too long",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, "took too long");
    }
}
