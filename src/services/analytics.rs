//! Analytics service: feature flags and event logging.
//!
//! Ported from ref/services/analytics/growthbook.ts` and
//! `ref/services/analytics/index.ts`. The TypeScript implementation uses the
//! GrowthBook SDK for feature flags and experiment assignment. This Rust port
//! provides the same interface backed by a simple in-memory feature store.
//! A production deployment would swap the store for the real GrowthBook SDK
//! or a remote config backend.

use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// AnalyticsService
// ---------------------------------------------------------------------------

/// Feature-flag and event-logging service.
///
/// Provides:
/// - Feature gate checks (boolean flags).
/// - Feature value retrieval (typed JSON values).
/// - Event logging (analytics events with metadata).
///
/// The default implementation uses an in-memory store seeded by configuration
/// or remote refresh. Replace the backing store for production use.
pub struct AnalyticsService {
    /// Feature flags: gate name -> JSON value.
    features: Mutex<HashMap<String, serde_json::Value>>,
    /// Logged events (buffered in memory for the session).
    events: Mutex<Vec<AnalyticsEvent>>,
}

/// A recorded analytics event.
#[derive(Debug, Clone)]
pub struct AnalyticsEvent {
    pub name: String,
    pub metadata: HashMap<String, String>,
}

impl AnalyticsService {
    /// Create a new service with no features loaded.
    pub fn new() -> Self {
        Self {
            features: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
        }
    }

    /// Create a service pre-loaded with the given feature flags.
    pub fn with_features(features: HashMap<String, serde_json::Value>) -> Self {
        Self {
            features: Mutex::new(features),
            events: Mutex::new(Vec::new()),
        }
    }

    // -- Feature flags -------------------------------------------------------

    /// Check a boolean feature gate.
    ///
    /// Returns `true` if the gate exists and is a truthy JSON value
    /// (`true`, a non-zero number, or a non-empty string).
    pub fn check_feature_gate(&self, gate: &str) -> bool {
        let features = self.features.lock().unwrap();
        match features.get(gate) {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::Number(n)) => n.as_f64().map_or(false, |v| v != 0.0),
            Some(serde_json::Value::String(s)) => !s.is_empty(),
            _ => false,
        }
    }

    /// Retrieve the raw JSON value for a feature flag.
    pub fn get_feature_value(&self, key: &str) -> Option<serde_json::Value> {
        let features = self.features.lock().unwrap();
        features.get(key).cloned()
    }

    /// Set or update a feature flag value.
    pub fn set_feature(&self, key: &str, value: serde_json::Value) {
        let mut features = self.features.lock().unwrap();
        features.insert(key.to_owned(), value);
    }

    /// Bulk-load feature flags, replacing any existing values.
    pub fn load_features(&self, new_features: HashMap<String, serde_json::Value>) {
        let mut features = self.features.lock().unwrap();
        *features = new_features;
    }

    // -- Event logging -------------------------------------------------------

    /// Log an analytics event with the given metadata.
    pub fn log_event(&self, event: &str, metadata: &HashMap<String, String>) {
        let mut events = self.events.lock().unwrap();
        events.push(AnalyticsEvent {
            name: event.to_owned(),
            metadata: metadata.clone(),
        });
        tracing::debug!(event, "analytics: logged event");
    }

    /// Retrieve all logged events (drains the buffer).
    pub fn drain_events(&self) -> Vec<AnalyticsEvent> {
        let mut events = self.events.lock().unwrap();
        std::mem::take(&mut *events)
    }

    /// Number of events currently buffered.
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

impl Default for AnalyticsService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AnalyticsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let feature_count = self.features.lock().map(|f| f.len()).unwrap_or(0);
        let event_count = self.events.lock().map(|e| e.len()).unwrap_or(0);
        f.debug_struct("AnalyticsService")
            .field("features", &feature_count)
            .field("buffered_events", &event_count)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_gate_true() {
        let svc = AnalyticsService::new();
        svc.set_feature("my_gate", serde_json::Value::Bool(true));
        assert!(svc.check_feature_gate("my_gate"));
    }

    #[test]
    fn check_gate_false() {
        let svc = AnalyticsService::new();
        svc.set_feature("my_gate", serde_json::Value::Bool(false));
        assert!(!svc.check_feature_gate("my_gate"));
    }

    #[test]
    fn check_gate_missing() {
        let svc = AnalyticsService::new();
        assert!(!svc.check_feature_gate("nonexistent"));
    }

    #[test]
    fn get_feature_value_json() {
        let svc = AnalyticsService::new();
        svc.set_feature("config", serde_json::json!({"threshold": 100}));
        let val = svc.get_feature_value("config").unwrap();
        assert_eq!(val["threshold"], 100);
    }

    #[test]
    fn log_and_drain_events() {
        let svc = AnalyticsService::new();
        let mut meta = HashMap::new();
        meta.insert("key".to_owned(), "value".to_owned());
        svc.log_event("test_event", &meta);
        svc.log_event("test_event_2", &HashMap::new());

        assert_eq!(svc.event_count(), 2);
        let events = svc.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "test_event");
        assert_eq!(svc.event_count(), 0);
    }
}
