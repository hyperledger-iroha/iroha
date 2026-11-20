//! Cumulative ingress latency at the same millisecond precision as the progress report.

use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct SubmitLatency {
    samples: u64,
    counts: BTreeMap<u64, u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct LatencySummary {
    pub(super) samples: u64,
    pub(super) p50_ms: u64,
    pub(super) p95_ms: u64,
    pub(super) p99_ms: u64,
    pub(super) max_ms: u64,
}

impl SubmitLatency {
    pub(super) fn record(&mut self, ms: u64, transactions: u64) {
        if transactions != 0 {
            *self.counts.entry(ms).or_default() += transactions;
            self.samples += transactions;
        }
    }

    pub(super) fn summary(&self) -> LatencySummary {
        let Some((&max_ms, _)) = self.counts.last_key_value() else {
            return LatencySummary::default();
        };
        // ceil(n * p/100), using integer ranks for p = 50, 95 and 99.
        let n = self.samples;
        let ranks = [n - n / 2, n - n / 20, n - n / 100];
        let mut values = [0; 3];
        let mut next = 0;
        let mut observed = 0;
        for (&ms, &count) in &self.counts {
            observed += count;
            while next < ranks.len() && observed >= ranks[next] {
                values[next] = ms;
                next += 1;
            }
            if next == ranks.len() {
                break;
            }
        }
        LatencySummary {
            samples: n,
            p50_ms: values[0],
            p95_ms: values[1],
            p99_ms: values[2],
            max_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_snapshots_match_sorted_nearest_ranks() {
        let mut histogram = SubmitLatency::default();
        let mut samples = Vec::new();
        assert_eq!(histogram.summary(), LatencySummary::default());
        for i in 0..257 {
            let value = match i % 6 {
                0 => u64::MAX,
                1 => 0,
                2 => 1_000,
                _ => (i * 17) % 41,
            };
            histogram.record(value, 1);
            samples.push(value);
            samples.sort_unstable();
            let at = |p: usize| samples[(samples.len() * p).div_ceil(100) - 1];
            assert_eq!(
                histogram.summary(),
                LatencySummary {
                    samples: samples.len() as u64,
                    p50_ms: at(50),
                    p95_ms: at(95),
                    p99_ms: at(99),
                    max_ms: *samples.last().unwrap(),
                }
            );
        }
    }

    #[test]
    fn batch_weights_match_individual_observations_after_every_batch() {
        let mut weighted = SubmitLatency::default();
        let mut individual = SubmitLatency::default();
        for (ms, transactions) in [
            (u64::MAX, 0),
            (0, 32),
            (52, 1),
            (77, 32),
            (1, 31),
            (77, 4),
            (u64::MAX, 1),
            (6, 0),
        ] {
            weighted.record(ms, transactions);
            for _ in 0..transactions {
                individual.record(ms, 1);
            }
            assert_eq!(weighted.counts, individual.counts);
            assert_eq!(weighted.summary(), individual.summary());
        }
        assert_eq!(
            weighted.summary(),
            LatencySummary {
                samples: 101,
                p50_ms: 1,
                p95_ms: 77,
                p99_ms: 77,
                max_ms: u64::MAX,
            }
        );
    }
}
