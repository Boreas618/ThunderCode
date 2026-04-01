//! Performance metrics with reservoir-sampled percentile estimation.
//!
//! `Stats` tracks count, min, max, sum, and sum-of-squares for O(1)
//! recording plus a fixed-size reservoir sample for approximate
//! percentile queries (p50, p95, p99).

use rand::Rng;

/// Maximum number of samples kept in the reservoir.
const RESERVOIR_SIZE: usize = 1024;

/// Streaming statistics tracker with reservoir sampling for percentiles.
///
/// Uses [Vitter's Algorithm R](https://en.wikipedia.org/wiki/Reservoir_sampling)
/// to maintain a uniform random sample of at most `RESERVOIR_SIZE` values,
/// from which approximate percentiles are computed on demand.
#[derive(Debug, Clone)]
pub struct Stats {
    count: u64,
    min: f64,
    max: f64,
    sum: f64,
    sum_squares: f64,
    /// Reservoir sample for percentile estimation.
    reservoir: Vec<f64>,
}

impl Stats {
    /// Create a new, empty `Stats` tracker.
    pub fn new() -> Self {
        Self {
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            sum: 0.0,
            sum_squares: 0.0,
            reservoir: Vec::with_capacity(RESERVOIR_SIZE),
        }
    }

    /// Record a new observation.
    pub fn record(&mut self, value: f64) {
        self.count += 1;
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
        self.sum += value;
        self.sum_squares += value * value;

        // Reservoir sampling (Algorithm R)
        if self.reservoir.len() < RESERVOIR_SIZE {
            self.reservoir.push(value);
        } else {
            // Replace a random element with decreasing probability
            let j = rand::thread_rng().gen_range(0..self.count as usize);
            if j < RESERVOIR_SIZE {
                self.reservoir[j] = value;
            }
        }
    }

    /// Number of values recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Minimum observed value (returns `f64::INFINITY` if empty).
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Maximum observed value (returns `f64::NEG_INFINITY` if empty).
    pub fn max(&self) -> f64 {
        self.max
    }

    /// Arithmetic mean. Returns `0.0` for an empty tracker.
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Sum of all recorded values.
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// Approximate median (50th percentile).
    pub fn p50(&self) -> f64 {
        self.percentile(0.50)
    }

    /// Approximate 95th percentile.
    pub fn p95(&self) -> f64 {
        self.percentile(0.95)
    }

    /// Approximate 99th percentile.
    pub fn p99(&self) -> f64 {
        self.percentile(0.99)
    }

    /// Approximate percentile from the reservoir sample.
    ///
    /// `p` must be in `[0.0, 1.0]`.  Returns `0.0` if no data has been
    /// recorded.  Uses nearest-rank interpolation on a sorted copy of the
    /// reservoir.  Percentile queries are expected to be infrequent relative
    /// to `record` calls so the sort cost is acceptable.
    pub fn percentile(&self, p: f64) -> f64 {
        if self.reservoir.is_empty() {
            return 0.0;
        }
        let mut sorted = self.reservoir.clone();
        sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p * (sorted.len() as f64 - 1.0)).round()) as usize;
        let idx = idx.min(sorted.len() - 1);
        sorted[idx]
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_stats() {
        let s = Stats::new();
        assert_eq!(s.count(), 0);
        assert_eq!(s.avg(), 0.0);
        assert_eq!(s.p50(), 0.0);
        assert_eq!(s.p95(), 0.0);
        assert_eq!(s.p99(), 0.0);
        assert_eq!(s.min(), f64::INFINITY);
        assert_eq!(s.max(), f64::NEG_INFINITY);
    }

    #[test]
    fn test_single_value() {
        let mut s = Stats::new();
        s.record(42.0);
        assert_eq!(s.count(), 1);
        assert!((s.min() - 42.0).abs() < f64::EPSILON);
        assert!((s.max() - 42.0).abs() < f64::EPSILON);
        assert!((s.avg() - 42.0).abs() < f64::EPSILON);
        assert!((s.p50() - 42.0).abs() < f64::EPSILON);
        assert!((s.p99() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_multiple_values() {
        let mut s = Stats::new();
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            s.record(v);
        }
        assert_eq!(s.count(), 5);
        assert!((s.min() - 1.0).abs() < f64::EPSILON);
        assert!((s.max() - 5.0).abs() < f64::EPSILON);
        assert!((s.avg() - 3.0).abs() < f64::EPSILON);
        assert!((s.sum() - 15.0).abs() < f64::EPSILON);

        // With 5 values, p50 should be close to 3.0
        let p50 = s.p50();
        assert!((p50 - 3.0).abs() < f64::EPSILON, "p50 was {}", p50);
    }

    #[test]
    fn test_percentiles_sorted_data() {
        let mut s = Stats::new();
        // Record 0..=99 -> 100 values
        for i in 0..100 {
            s.record(i as f64);
        }
        assert_eq!(s.count(), 100);

        // p50 should be near 49-50
        let p50 = s.p50();
        assert!(
            (45.0..=55.0).contains(&p50),
            "p50 = {} not in expected range",
            p50
        );

        // p95 should be near 94-95
        let p95 = s.p95();
        assert!(
            (90.0..=99.0).contains(&p95),
            "p95 = {} not in expected range",
            p95
        );

        // p99 should be near 98-99
        let p99 = s.p99();
        assert!(
            (95.0..=99.0).contains(&p99),
            "p99 = {} not in expected range",
            p99
        );
    }

    #[test]
    fn test_reservoir_sampling_large_dataset() {
        let mut s = Stats::new();
        // Record 10,000 values -- well beyond the reservoir size
        for i in 0..10_000 {
            s.record(i as f64);
        }
        assert_eq!(s.count(), 10_000);
        assert!((s.min() - 0.0).abs() < f64::EPSILON);
        assert!((s.max() - 9_999.0).abs() < f64::EPSILON);

        // The reservoir sample should give reasonable percentile estimates.
        // With 10k uniform values, p50 ~ 5000, p95 ~ 9500.
        // Allow wide tolerance since it is approximate.
        let p50 = s.p50();
        assert!(
            (3_000.0..=7_000.0).contains(&p50),
            "p50 = {} out of range for 10k uniform",
            p50
        );

        let p95 = s.p95();
        assert!(
            (8_500.0..=10_000.0).contains(&p95),
            "p95 = {} out of range for 10k uniform",
            p95
        );
    }

    #[test]
    fn test_default_impl() {
        let s = Stats::default();
        assert_eq!(s.count(), 0);
    }
}
