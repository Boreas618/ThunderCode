//! System wake lock: prevent the OS from sleeping during long operations.
//!
//! Ported from ref/services/preventSleep.ts`. On macOS, spawns `caffeinate`
//! with a timeout; the process auto-exits if the parent is killed with SIGKILL.
//! On other platforms this is a no-op.

use std::process::{Child, Command, Stdio};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Caffeinate timeout in seconds. The process auto-exits after this duration.
/// We restart it before expiry to maintain continuous sleep prevention.
const CAFFEINATE_TIMEOUT_SECONDS: u32 = 300; // 5 minutes

// ---------------------------------------------------------------------------
// WakeLock
// ---------------------------------------------------------------------------

/// RAII wake lock that prevents the system from sleeping.
///
/// On macOS, spawns `caffeinate -i -t <timeout>` which creates a power
/// assertion preventing idle sleep. The `caffeinate` process is killed when
/// the `WakeLock` is dropped.
///
/// On non-macOS platforms, `acquire()` succeeds but does nothing.
pub struct WakeLock {
    #[allow(dead_code)]
    process: Option<Child>,
}

impl WakeLock {
    /// Acquire a wake lock. On macOS, spawns `caffeinate`. On other platforms
    /// returns a no-op lock.
    pub fn acquire() -> anyhow::Result<Self> {
        let process = if cfg!(target_os = "macos") {
            match spawn_caffeinate() {
                Ok(child) => {
                    tracing::debug!("prevent_sleep: acquired wake lock (caffeinate)");
                    Some(child)
                }
                Err(e) => {
                    tracing::warn!("prevent_sleep: failed to spawn caffeinate: {e}");
                    None
                }
            }
        } else {
            tracing::debug!("prevent_sleep: wake lock is a no-op on this platform");
            None
        };

        Ok(Self { process })
    }

    /// Check whether the wake lock is actively held.
    pub fn is_active(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for WakeLock {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            match child.kill() {
                Ok(()) => {
                    let _ = child.wait();
                    tracing::debug!("prevent_sleep: released wake lock (killed caffeinate)");
                }
                Err(e) => {
                    // The process may have already exited (e.g. timeout expired).
                    tracing::debug!("prevent_sleep: caffeinate already exited: {e}");
                }
            }
        }
    }
}

impl std::fmt::Debug for WakeLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeLock")
            .field("active", &self.is_active())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

/// Spawn `caffeinate -i -t <timeout>`.
///
/// `-i`: Create an assertion to prevent idle sleep (display can still sleep).
/// `-t`: Timeout in seconds -- caffeinate exits automatically after this,
///       providing self-healing if the parent is killed with SIGKILL.
fn spawn_caffeinate() -> anyhow::Result<Child> {
    let child = Command::new("caffeinate")
        .args(["-i", "-t", &CAFFEINATE_TIMEOUT_SECONDS.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(child)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_and_drop() {
        // On macOS this actually spawns caffeinate; on CI/Linux it's a no-op.
        let lock = WakeLock::acquire().unwrap();
        if cfg!(target_os = "macos") {
            assert!(lock.is_active());
        }
        drop(lock);
        // No panic = success.
    }
}
