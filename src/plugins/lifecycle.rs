//! Plugin lifecycle management.
//!
//! Defines the `PluginLifecycle` trait that plugins implement to hook into
//! init / activate / deactivate phases.

use anyhow::Result;
use async_trait::async_trait;

// ---------------------------------------------------------------------------
// PluginLifecycle
// ---------------------------------------------------------------------------

/// Lifecycle hooks for a plugin.
///
/// Implementors can perform setup work during [`init`](PluginLifecycle::init),
/// enable their functionality in [`activate`](PluginLifecycle::activate), and
/// clean up in [`deactivate`](PluginLifecycle::deactivate).
#[async_trait]
pub trait PluginLifecycle: Send + Sync {
    /// Called once when the plugin is first loaded.
    ///
    /// Use this for one-time setup such as validating configuration or
    /// registering resources that persist across enable/disable cycles.
    async fn init(&self) -> Result<()> {
        Ok(())
    }

    /// Called when the plugin is enabled (or on startup if already enabled).
    ///
    /// Use this to register skills, hooks, MCP servers, or other components
    /// that should only be active while the plugin is enabled.
    async fn activate(&self) -> Result<()> {
        Ok(())
    }

    /// Called when the plugin is disabled or the CLI is shutting down.
    ///
    /// Use this to clean up any resources registered during [`activate`](Self::activate).
    async fn deactivate(&self) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// NoopLifecycle
// ---------------------------------------------------------------------------

/// A no-op lifecycle implementation for plugins that need no special handling.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopLifecycle;

#[async_trait]
impl PluginLifecycle for NoopLifecycle {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct TrackingLifecycle {
        init_count: Arc<AtomicU32>,
        activate_count: Arc<AtomicU32>,
        deactivate_count: Arc<AtomicU32>,
    }

    #[async_trait]
    impl PluginLifecycle for TrackingLifecycle {
        async fn init(&self) -> Result<()> {
            self.init_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn activate(&self) -> Result<()> {
            self.activate_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn deactivate(&self) -> Result<()> {
            self.deactivate_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_noop_lifecycle() {
        let noop = NoopLifecycle;
        assert!(noop.init().await.is_ok());
        assert!(noop.activate().await.is_ok());
        assert!(noop.deactivate().await.is_ok());
    }

    #[tokio::test]
    async fn test_tracking_lifecycle() {
        let init_count = Arc::new(AtomicU32::new(0));
        let activate_count = Arc::new(AtomicU32::new(0));
        let deactivate_count = Arc::new(AtomicU32::new(0));

        let lifecycle = TrackingLifecycle {
            init_count: Arc::clone(&init_count),
            activate_count: Arc::clone(&activate_count),
            deactivate_count: Arc::clone(&deactivate_count),
        };

        lifecycle.init().await.unwrap();
        assert_eq!(init_count.load(Ordering::SeqCst), 1);
        assert_eq!(activate_count.load(Ordering::SeqCst), 0);

        lifecycle.activate().await.unwrap();
        lifecycle.activate().await.unwrap();
        assert_eq!(activate_count.load(Ordering::SeqCst), 2);

        lifecycle.deactivate().await.unwrap();
        assert_eq!(deactivate_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_lifecycle_as_trait_object() {
        let lifecycle: Box<dyn PluginLifecycle> = Box::new(NoopLifecycle);
        assert!(lifecycle.init().await.is_ok());
        assert!(lifecycle.activate().await.is_ok());
        assert!(lifecycle.deactivate().await.is_ok());
    }
}
