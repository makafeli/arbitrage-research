//! Fixed-memory latency populations for operational scheduler observations.
//! Percentiles are conservative bucket upper bounds, not exact quantiles or SLAs.
use std::time::Duration;

// Bucket zero contains exactly zero; bucket i contains [2^(i-1), 2^i - 1]
// nanoseconds. Duration::MAX fits within 94 bits, including its subsecond part.
const BUCKETS: usize = 96;

/// Cumulative, fixed-cardinality duration summary. None means no measurement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LatencySummary {
    pub samples: u64,
    pub saturated: bool,
    pub min_ns: Option<u128>,
    pub max_ns: Option<u128>,
    pub p50_upper_ns: Option<u128>,
    pub p95_upper_ns: Option<u128>,
    pub p99_upper_ns: Option<u128>,
}

/// Queue waits are recorded only for dispatched work. Execution populations
/// include only explicitly finished permits, classified by scheduler admission.
/// Abandoned permits have no fabricated duration or success measurement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StageTimingSnapshot {
    pub queue_wait: LatencySummary,
    pub successful_execution: LatencySummary,
    pub rejected_execution: LatencySummary,
    pub unmeasurable_completions: u64,
}

/// Reusable fixed-memory duration population; see LatencySummary for bound semantics.
#[derive(Clone)]
pub struct Histogram {
    buckets: [u64; BUCKETS],
    samples: u64,
    saturated: bool,
    min_ns: Option<u128>,
    max_ns: Option<u128>,
}
impl Default for Histogram {
    fn default() -> Self {
        Self {
            buckets: [0; BUCKETS],
            samples: 0,
            saturated: false,
            min_ns: None,
            max_ns: None,
        }
    }
}
impl Histogram {
    /// O(1) insertion without allocation. Freeze the entire population on count
    /// exhaustion so bucket counts and the percentile denominator never diverge.
    pub fn record(&mut self, duration: Duration) {
        if self.samples == u64::MAX {
            self.saturated = true;
            return;
        }
        let ns = duration.as_nanos();
        let index = (u128::BITS - ns.leading_zeros()) as usize;
        self.buckets[index] += 1;
        self.samples += 1;
        self.min_ns = Some(self.min_ns.map_or(ns, |old| old.min(ns)));
        self.max_ns = Some(self.max_ns.map_or(ns, |old| old.max(ns)));
    }

    fn upper_bound(&self, percentile: u128) -> Option<u128> {
        if self.samples == 0 {
            return None;
        }
        let rank = (u128::from(self.samples) * percentile).div_ceil(100);
        let mut cumulative = 0_u128;
        for (index, count) in self.buckets.iter().enumerate() {
            cumulative += u128::from(*count);
            if cumulative >= rank {
                let upper = (1_u128 << index) - 1;
                // The exact maximum safely tightens the last occupied bucket.
                return Some(upper.min(self.max_ns.expect("nonempty histogram")));
            }
        }
        unreachable!("histogram population equals the sum of its buckets")
    }

    /// Copy conservative integer percentile bounds without retaining raw samples.
    pub fn snapshot(&self) -> LatencySummary {
        LatencySummary {
            samples: self.samples,
            saturated: self.saturated,
            min_ns: self.min_ns,
            max_ns: self.max_ns,
            p50_upper_ns: self.upper_bound(50),
            p95_upper_ns: self.upper_bound(95),
            p99_upper_ns: self.upper_bound(99),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct StageTiming {
    pub(crate) queue_wait: Histogram,
    pub(crate) successful_execution: Histogram,
    pub(crate) rejected_execution: Histogram,
    pub(crate) unmeasurable_completions: u64,
}
impl StageTiming {
    pub(crate) fn snapshot(&self) -> StageTimingSnapshot {
        StageTimingSnapshot {
            queue_wait: self.queue_wait.snapshot(),
            successful_execution: self.successful_execution.snapshot(),
            rejected_execution: self.rejected_execution.snapshot(),
            unmeasurable_completions: self.unmeasurable_completions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_known_zero_are_not_interchangeable() {
        let mut histogram = Histogram::default();
        assert_eq!(histogram.snapshot(), LatencySummary::default());
        histogram.record(Duration::ZERO);
        let summary = histogram.snapshot();
        assert_eq!(summary.samples, 1);
        assert_eq!(summary.min_ns, Some(0));
        assert_eq!(summary.p99_upper_ns, Some(0));
    }

    #[test]
    fn bucket_edges_are_conservative_and_maximum_is_exact() {
        let mut histogram = Histogram::default();
        for ns in 1..=100 {
            histogram.record(Duration::from_nanos(ns));
        }
        let summary = histogram.snapshot();
        assert_eq!(summary.samples, 100);
        assert_eq!(summary.min_ns, Some(1));
        assert_eq!(summary.max_ns, Some(100));
        assert_eq!(summary.p50_upper_ns, Some(63));
        assert_eq!(summary.p95_upper_ns, Some(100));
        assert_eq!(summary.p99_upper_ns, Some(100));
        for ns in [0, 1, 2, 3, 4, 7, 8, 15, 16, 1023, 1024] {
            let mut single = Histogram::default();
            single.record(Duration::from_nanos(ns));
            assert_eq!(single.snapshot().p50_upper_ns, Some(u128::from(ns)));
        }
    }

    #[test]
    fn exact_nearest_rank_never_exceeds_reported_upper_bound() {
        let mut histogram = Histogram::default();
        let mut values = Vec::new();
        for i in 0..10_001_u64 {
            let value = (i * 7_919) % 65_537;
            values.push(value);
            histogram.record(Duration::from_nanos(value));
        }
        values.sort_unstable();
        let summary = histogram.snapshot();
        for (percent, bound) in [
            (50, summary.p50_upper_ns),
            (95, summary.p95_upper_ns),
            (99, summary.p99_upper_ns),
        ] {
            let rank = (values.len() * percent).div_ceil(100);
            assert!(bound.unwrap() >= u128::from(values[rank - 1]));
            assert!(bound.unwrap() <= u128::from(*values.last().unwrap()));
        }
    }

    #[test]
    fn largest_duration_and_repeated_samples_keep_fixed_storage() {
        let mut histogram = Histogram::default();
        let bytes = std::mem::size_of_val(&histogram);
        for _ in 0..100_000 {
            histogram.record(Duration::MAX);
        }
        assert_eq!(
            histogram.snapshot().p99_upper_ns,
            Some(Duration::MAX.as_nanos())
        );
        assert_eq!(histogram.snapshot().samples, 100_000);
        assert_eq!(std::mem::size_of_val(&histogram), bytes);
        assert!(bytes < 1024);
    }

    #[test]
    fn count_saturation_freezes_every_part_of_the_population() {
        let mut buckets = [0; BUCKETS];
        buckets[0] = u64::MAX - 1;
        let mut histogram = Histogram {
            buckets,
            samples: u64::MAX - 1,
            saturated: false,
            min_ns: Some(0),
            max_ns: Some(0),
        };
        histogram.record(Duration::ZERO);
        histogram.record(Duration::MAX);
        let summary = histogram.snapshot();
        assert!(summary.saturated);
        assert_eq!(summary.samples, u64::MAX);
        assert_eq!(summary.max_ns, Some(0));
        assert_eq!(summary.p99_upper_ns, Some(0));
    }
}
