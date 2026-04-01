//! Cost tracking for API usage.
//!
//! Tracks per-call token usage and cost, provides per-model breakdowns.
//! Pricing is not hardcoded -- callers supply cost_usd per entry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Calculate USD cost for an API call.
///
/// Returns 0.0 -- pricing is provider-specific and not built in.
/// Callers can supply their own cost_usd in CostEntry if they have pricing info.
pub fn calculate_cost(
    _model: &str,
    _input_tokens: u64,
    _output_tokens: u64,
    _cache_read_tokens: u64,
    _cache_write_tokens: u64,
) -> f64 {
    0.0
}

// ---------------------------------------------------------------------------
// CostEntry
// ---------------------------------------------------------------------------

/// A single recorded API call with token usage and cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
    pub duration_ms: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// ModelCosts -- aggregated per-model statistics
// ---------------------------------------------------------------------------

/// Aggregated token counts and cost for a single model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCosts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
    pub call_count: u64,
}

// ---------------------------------------------------------------------------
// CostSummary
// ---------------------------------------------------------------------------

/// Full session cost summary including per-model breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_write_tokens: u64,
    pub model_breakdown: HashMap<String, ModelCosts>,
}

// ---------------------------------------------------------------------------
// CostTracker
// ---------------------------------------------------------------------------

/// Accumulates API call cost entries and provides summaries.
#[derive(Debug, Clone)]
pub struct CostTracker {
    entries: Vec<CostEntry>,
    total_cost_usd: f64,
    total_duration_ms: u64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total_cost_usd: 0.0,
            total_duration_ms: 0,
        }
    }

    pub fn record(&mut self, entry: CostEntry) {
        self.total_cost_usd += entry.cost_usd;
        self.total_duration_ms += entry.duration_ms;
        self.entries.push(entry);
    }

    pub fn total_cost(&self) -> f64 {
        self.total_cost_usd
    }

    pub fn total_duration(&self) -> u64 {
        self.total_duration_ms
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[CostEntry] {
        &self.entries
    }

    pub fn summary(&self) -> CostSummary {
        let breakdown = self.per_model_breakdown();

        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut total_cache_read = 0u64;
        let mut total_cache_write = 0u64;

        for mc in breakdown.values() {
            total_input += mc.input_tokens;
            total_output += mc.output_tokens;
            total_cache_read += mc.cache_read_tokens;
            total_cache_write += mc.cache_write_tokens;
        }

        CostSummary {
            total_cost_usd: self.total_cost_usd,
            total_duration_ms: self.total_duration_ms,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_cache_read_tokens: total_cache_read,
            total_cache_write_tokens: total_cache_write,
            model_breakdown: breakdown,
        }
    }

    pub fn per_model_breakdown(&self) -> HashMap<String, ModelCosts> {
        let mut map: HashMap<String, ModelCosts> = HashMap::new();
        for entry in &self.entries {
            let mc = map.entry(entry.model.clone()).or_default();
            mc.input_tokens += entry.input_tokens;
            mc.output_tokens += entry.output_tokens;
            mc.cache_read_tokens += entry.cache_read_tokens;
            mc.cache_write_tokens += entry.cache_write_tokens;
            mc.cost_usd += entry.cost_usd;
            mc.call_count += 1;
        }
        map
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_record_and_summary() {
        let mut tracker = CostTracker::new();

        tracker.record(CostEntry {
            model: "gpt-4o".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: 0.01,
            duration_ms: 100,
            timestamp: 1000,
        });

        tracker.record(CostEntry {
            model: "gpt-4o-mini".to_string(),
            input_tokens: 2000,
            output_tokens: 1000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: 0.005,
            duration_ms: 200,
            timestamp: 2000,
        });

        assert_eq!(tracker.entry_count(), 2);
        let summary = tracker.summary();
        assert_eq!(summary.total_input_tokens, 3000);
        assert_eq!(summary.total_output_tokens, 1500);
        assert_eq!(summary.model_breakdown.len(), 2);
    }

    #[test]
    fn test_tracker_empty() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.entry_count(), 0);
        let summary = tracker.summary();
        assert!(summary.model_breakdown.is_empty());
    }
}
