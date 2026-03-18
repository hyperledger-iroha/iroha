//! PDP/PoTR simulation harness shared across integration tests and tooling.
use std::time::Duration;

use norito::json::{Map, Value};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Deterministic seed used for standard reports.
pub const DEFAULT_SEED: u64 = 0x5eed;

/// Configuration knobs for the PDP/PoTR simulator.
#[derive(Clone, Copy, Debug)]
pub struct SimulationConfig {
    /// Total number of storage nodes participating in the simulation.
    pub nodes: usize,
    /// Number of adversarial nodes within the cluster (must be < `nodes`).
    pub adversarial: usize,
    /// Number of geographic regions that nodes are partitioned into.
    pub regions: usize,
    /// Maximum number of regions that can be isolated during a partition.
    pub max_partition_regions: usize,
    /// Number of epochs to simulate.
    pub epochs: usize,
    /// Number of PDP challenge rounds executed per epoch.
    pub pdp_challenges_per_epoch: usize,
    /// Number of `PoTR` windows executed per epoch.
    pub potr_windows_per_epoch: usize,
    /// Interval between challenges, used to derive latency samples.
    pub challenge_interval: Duration,
    /// Probability that any epoch experiences a partition at all.
    pub partition_probability: f64,
    /// Failure probability for adversarial nodes during challenges.
    pub adversary_failure_probability: f64,
    /// Failure probability for honest nodes during challenges.
    pub honest_network_failure_probability: f64,
    /// Number of consecutive late `PoTR` epochs required before detection.
    pub potr_late_threshold: usize,
    /// Detection bias applied when no partition is active.
    pub detection_bias_no_partition: f64,
    /// Maximum repairs that can be completed per epoch.
    pub repair_capacity_per_epoch: usize,
}

impl SimulationConfig {
    /// Validates invariants required by the simulator.
    pub fn validate(&self) {
        assert!(self.nodes > 0, "simulation must include nodes");
        assert!(
            self.adversarial < self.nodes,
            "need at least one honest node"
        );
        assert!(self.regions > 0, "at least one region is required");
        assert!(
            self.max_partition_regions > 0 && self.max_partition_regions <= self.regions,
            "partition regions must be within the configured region count"
        );
        assert!(
            (0.0..=1.0).contains(&self.partition_probability),
            "partition probability must be in [0, 1]"
        );
        assert!(
            (0.0..=1.0).contains(&self.adversary_failure_probability),
            "adversary failure probability must be in [0, 1]"
        );
        assert!(
            (0.0..=1.0).contains(&self.honest_network_failure_probability),
            "honest failure probability must be in [0, 1]"
        );
        assert!(
            (0.0..=1.0).contains(&self.detection_bias_no_partition),
            "detection bias must be in [0, 1]"
        );
        assert!(self.potr_late_threshold > 0, "PoTR threshold must be > 0");
        assert!(
            self.repair_capacity_per_epoch > 0,
            "repair capacity must be > 0"
        );
    }

    fn json_value(self) -> Value {
        let interval_ms = u64::try_from(self.challenge_interval.as_millis()).unwrap_or(u64::MAX);
        json_object(vec![
            ("nodes", Value::from(self.nodes)),
            ("adversarial", Value::from(self.adversarial)),
            ("regions", Value::from(self.regions)),
            (
                "max_partition_regions",
                Value::from(self.max_partition_regions),
            ),
            ("epochs", Value::from(self.epochs)),
            (
                "pdp_challenges_per_epoch",
                Value::from(self.pdp_challenges_per_epoch),
            ),
            (
                "potr_windows_per_epoch",
                Value::from(self.potr_windows_per_epoch),
            ),
            ("challenge_interval_ms", Value::from(interval_ms)),
            (
                "partition_probability",
                Value::from(self.partition_probability),
            ),
            (
                "adversary_failure_probability",
                Value::from(self.adversary_failure_probability),
            ),
            (
                "honest_network_failure_probability",
                Value::from(self.honest_network_failure_probability),
            ),
            ("potr_late_threshold", Value::from(self.potr_late_threshold)),
            (
                "detection_bias_no_partition",
                Value::from(self.detection_bias_no_partition),
            ),
            (
                "repair_capacity_per_epoch",
                Value::from(self.repair_capacity_per_epoch),
            ),
        ])
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            nodes: 12,
            adversarial: 3,
            regions: 3,
            max_partition_regions: 1,
            epochs: 12,
            pdp_challenges_per_epoch: 18,
            potr_windows_per_epoch: 2,
            challenge_interval: Duration::from_secs(30),
            partition_probability: 0.2,
            adversary_failure_probability: 0.55,
            honest_network_failure_probability: 0.05,
            potr_late_threshold: 2,
            detection_bias_no_partition: 0.9,
            repair_capacity_per_epoch: 4,
        }
    }
}

#[derive(Clone, Debug)]
struct NodeState {
    _id: usize,
    region: usize,
    adversarial: bool,
    potr_late_epochs: usize,
}

/// Per-challenge statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChallengeStats {
    /// Total number of challenges processed.
    pub total: usize,
    /// Number of challenges that resulted in a failure.
    pub failures: usize,
    /// Number of failures that were detected.
    pub detected: usize,
    /// Number of failures that were not detected.
    pub undetected: usize,
    detection_latency_sum_epochs: u64,
    detection_latency_samples: u64,
}

impl ChallengeStats {
    /// Records a successful challenge outcome.
    pub fn record_success(&mut self) {
        self.total += 1;
    }

    /// Records a failure outcome along with whether it was detected and the latency.
    pub fn record_failure(&mut self, detected: bool, latency_epochs: u64) {
        self.total += 1;
        self.failures += 1;
        if detected {
            self.detected += 1;
            self.detection_latency_samples += 1;
            self.detection_latency_sum_epochs += latency_epochs;
        } else {
            self.undetected += 1;
        }
    }

    /// Fraction of failures that were detected.
    #[allow(clippy::cast_precision_loss)]
    pub fn detection_rate(&self) -> f64 {
        if self.failures == 0 {
            return 1.0;
        }
        self.detected as f64 / self.failures as f64
    }

    /// Mean number of epochs required to detect failures.
    #[allow(clippy::cast_precision_loss)]
    pub fn mean_detection_latency_epochs(&self) -> f64 {
        if self.detection_latency_samples == 0 {
            return 0.0;
        }
        self.detection_latency_sum_epochs as f64 / self.detection_latency_samples as f64
    }

    fn merge(&mut self, other: &Self) {
        self.total += other.total;
        self.failures += other.failures;
        self.detected += other.detected;
        self.undetected += other.undetected;
        self.detection_latency_sum_epochs += other.detection_latency_sum_epochs;
        self.detection_latency_samples += other.detection_latency_samples;
    }

    fn json_value(&self) -> Value {
        json_object(vec![
            ("total", Value::from(self.total)),
            ("failures", Value::from(self.failures)),
            ("detected", Value::from(self.detected)),
            ("undetected", Value::from(self.undetected)),
            ("detection_rate", Value::from(self.detection_rate())),
            (
                "mean_detection_latency_epochs",
                Value::from(self.mean_detection_latency_epochs()),
            ),
        ])
    }
}

/// `QoS` snapshot for a single epoch.
#[derive(Clone, Copy, Debug, Default)]
pub struct EpochQosSnapshot {
    /// Mean response latency across all samples in the epoch.
    pub mean_response_ms: f64,
    /// 95th percentile response latency for the epoch.
    pub p95_response_ms: f64,
    /// Maximum size reached by the repair queue during the epoch.
    pub repair_queue_peak: usize,
    /// Number of repair operations executed in the epoch.
    pub repair_events: usize,
}

impl EpochQosSnapshot {
    #[allow(clippy::cast_precision_loss)]
    fn from_samples(samples: &[u64], peak: usize, repair_events: usize) -> Self {
        let mean_response_ms = if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<u64>() as f64 / samples.len() as f64
        };
        let p95_response_ms = percentile(samples, 0.95);
        Self {
            mean_response_ms,
            p95_response_ms,
            repair_queue_peak: peak,
            repair_events,
        }
    }

    fn json_value(&self) -> Value {
        json_object(vec![
            ("mean_response_ms", Value::from(self.mean_response_ms)),
            ("p95_response_ms", Value::from(self.p95_response_ms)),
            ("repair_queue_peak", Value::from(self.repair_queue_peak)),
            ("repair_events", Value::from(self.repair_events)),
        ])
    }
}

/// Aggregated `QoS` statistics across all epochs.
#[derive(Clone, Debug, Default)]
pub struct AggregateQosStats {
    response_samples_ms: Vec<u64>,
    total_response_ms: u128,
    /// Maximum repair backlog observed across all epochs.
    pub repair_queue_peak: usize,
    /// Total number of repair operations performed across all epochs.
    pub total_repair_events: usize,
}

impl AggregateQosStats {
    fn absorb_samples(&mut self, mut samples: Vec<u64>) {
        self.total_response_ms += samples.iter().map(|value| u128::from(*value)).sum::<u128>();
        self.response_samples_ms.append(&mut samples);
    }

    fn record_repair_metrics(&mut self, peak: usize, repairs: usize) {
        self.repair_queue_peak = self.repair_queue_peak.max(peak);
        self.total_repair_events += repairs;
    }

    /// Mean latency across every recorded `QoS` sample.
    #[allow(clippy::cast_precision_loss)]
    pub fn mean_response_ms(&self) -> f64 {
        if self.response_samples_ms.is_empty() {
            return 0.0;
        }
        self.total_response_ms as f64 / self.response_samples_ms.len() as f64
    }

    /// 95th percentile latency aggregate for all epochs.
    pub fn p95_response_ms(&self) -> f64 {
        percentile(&self.response_samples_ms, 0.95)
    }

    fn json_value(&self) -> Value {
        json_object(vec![
            ("mean_response_ms", Value::from(self.mean_response_ms())),
            ("p95_response_ms", Value::from(self.p95_response_ms())),
            ("repair_queue_peak", Value::from(self.repair_queue_peak)),
            ("total_repair_events", Value::from(self.total_repair_events)),
        ])
    }
}

/// Per-epoch statistics captured by the simulator.
#[derive(Clone, Debug)]
pub struct EpochStats {
    /// Zero-based epoch index for reporting.
    pub index: usize,
    /// Regions that were partitioned during this epoch.
    pub partition_regions: Vec<usize>,
    /// Aggregate PDP stats for the epoch.
    pub pdp: ChallengeStats,
    /// Aggregate `PoTR` stats for the epoch.
    pub potr: ChallengeStats,
    /// `QoS` snapshot for the epoch.
    pub qos: EpochQosSnapshot,
    /// Total number of failures that were successfully detected.
    pub detected_failures: usize,
}

impl EpochStats {
    fn json_value(&self) -> Value {
        let regions = Value::Array(
            self.partition_regions
                .iter()
                .map(|region| Value::from(*region))
                .collect(),
        );
        json_object(vec![
            ("index", Value::from(self.index)),
            ("partition_regions", regions),
            ("detected_failures", Value::from(self.detected_failures)),
            ("pdp", self.pdp.json_value()),
            ("potr", self.potr.json_value()),
            ("qos", self.qos.json_value()),
        ])
    }
}

/// Aggregate statistics returned after running the simulator.
#[derive(Clone, Debug, Default)]
pub struct SimulationStats {
    /// Configuration used for the simulation run.
    pub config: SimulationConfig,
    /// Per-epoch statistics captured while running the simulation.
    pub epochs: Vec<EpochStats>,
    /// PDP totals aggregated across every epoch.
    pub aggregate_pdp: ChallengeStats,
    /// `PoTR` totals aggregated across every epoch.
    pub aggregate_potr: ChallengeStats,
    /// Aggregated `QoS` statistics across all epochs.
    pub qos: AggregateQosStats,
}

impl SimulationStats {
    /// Overall PDP detection rate across the entire simulation.
    pub fn pdp_detection_rate(&self) -> f64 {
        self.aggregate_pdp.detection_rate()
    }

    /// Overall `PoTR` detection rate across the entire simulation.
    pub fn potr_detection_rate(&self) -> f64 {
        self.aggregate_potr.detection_rate()
    }

    /// Emit a machine-readable JSON summary for dashboards and docs.
    pub fn json_summary(&self) -> Value {
        let epochs = self
            .epochs
            .iter()
            .map(EpochStats::json_value)
            .collect::<Vec<_>>();
        let aggregate = json_object(vec![
            ("pdp", self.aggregate_pdp.json_value()),
            ("potr", self.aggregate_potr.json_value()),
            ("qos", self.qos.json_value()),
        ]);
        json_object(vec![
            ("config", self.config.json_value()),
            ("aggregate", aggregate),
            ("epochs", Value::Array(epochs)),
        ])
    }
}

struct SimulationHarness {
    config: SimulationConfig,
    nodes: Vec<NodeState>,
    repair_backlog: usize,
}

impl SimulationHarness {
    fn new(config: SimulationConfig) -> Self {
        config.validate();
        let mut nodes = Vec::with_capacity(config.nodes);
        for id in 0..config.nodes {
            nodes.push(NodeState {
                _id: id,
                region: id % config.regions,
                adversarial: id < config.adversarial,
                potr_late_epochs: 0,
            });
        }
        Self {
            config,
            nodes,
            repair_backlog: 0,
        }
    }

    fn run(&mut self, seed: u64) -> SimulationStats {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut stats = SimulationStats {
            config: self.config,
            ..SimulationStats::default()
        };

        for epoch_idx in 0..self.config.epochs {
            let (mut epoch_stats, response_samples) = self.simulate_epoch(epoch_idx, &mut rng);
            stats.aggregate_pdp.merge(&epoch_stats.pdp);
            stats.aggregate_potr.merge(&epoch_stats.potr);
            stats.qos.absorb_samples(response_samples);
            stats.qos.record_repair_metrics(
                epoch_stats.qos.repair_queue_peak,
                epoch_stats.qos.repair_events,
            );
            epoch_stats.detected_failures = epoch_stats.pdp.detected + epoch_stats.potr.detected;
            stats.epochs.push(epoch_stats);
        }

        stats
    }

    fn simulate_epoch(&mut self, epoch_idx: usize, rng: &mut ChaCha8Rng) -> (EpochStats, Vec<u64>) {
        let partition_regions = self.sample_partition_regions(rng);
        let mut epoch_stats = EpochStats {
            index: epoch_idx,
            partition_regions: partition_regions.clone(),
            pdp: ChallengeStats::default(),
            potr: ChallengeStats::default(),
            qos: EpochQosSnapshot::default(),
            detected_failures: 0,
        };
        let mut response_samples = Vec::new();
        let mut epoch_repair_peak = self.repair_backlog;
        let mut epoch_repairs = 0;

        for _ in 0..self.config.pdp_challenges_per_epoch {
            self.simulate_pdp_challenge(
                &partition_regions,
                rng,
                &mut epoch_stats,
                &mut response_samples,
                &mut epoch_repair_peak,
                &mut epoch_repairs,
            );
        }

        for _ in 0..self.config.potr_windows_per_epoch {
            self.simulate_potr_window(
                &partition_regions,
                rng,
                &mut epoch_stats,
                &mut response_samples,
                &mut epoch_repair_peak,
                &mut epoch_repairs,
            );
        }

        epoch_repairs += self.drain_repairs();
        epoch_repair_peak = epoch_repair_peak.max(self.repair_backlog);

        epoch_stats.qos =
            EpochQosSnapshot::from_samples(&response_samples, epoch_repair_peak, epoch_repairs);

        (epoch_stats, response_samples)
    }

    fn simulate_pdp_challenge(
        &mut self,
        partition_regions: &[usize],
        rng: &mut ChaCha8Rng,
        epoch_stats: &mut EpochStats,
        response_samples: &mut Vec<u64>,
        epoch_repair_peak: &mut usize,
        epoch_repairs: &mut usize,
    ) {
        let target_idx = rng.random_range(0..self.nodes.len());
        let node = &self.nodes[target_idx];
        let partitioned = Self::node_partitioned(node, partition_regions);
        let latency_ms = self.sample_latency_ms(rng);
        response_samples.push(latency_ms);

        let failure = self.challenge_fails(node, partitioned, rng);
        if failure {
            let detected = self.detect_failure(partitioned, rng);
            epoch_stats.pdp.record_failure(detected, 0);
            if detected {
                self.repair_backlog += 1;
                *epoch_repairs += 1;
                *epoch_repair_peak = (*epoch_repair_peak).max(self.repair_backlog);
            }
        } else {
            epoch_stats.pdp.record_success();
        }
    }

    fn simulate_potr_window(
        &mut self,
        partition_regions: &[usize],
        rng: &mut ChaCha8Rng,
        epoch_stats: &mut EpochStats,
        response_samples: &mut Vec<u64>,
        epoch_repair_peak: &mut usize,
        epoch_repairs: &mut usize,
    ) {
        let node_count = self.nodes.len();
        for idx in 0..node_count {
            let latency_ms = self.sample_latency_ms(rng);
            response_samples.push(latency_ms);
            let partitioned = {
                let node = &self.nodes[idx];
                Self::node_partitioned(node, partition_regions)
            };
            let failure = {
                let node = &self.nodes[idx];
                self.potr_fails(node, partitioned, rng)
            };
            let node = &mut self.nodes[idx];
            if failure {
                node.potr_late_epochs += 1;
                let detected = node.potr_late_epochs >= self.config.potr_late_threshold;
                let latency_epochs = node.potr_late_epochs as u64;
                epoch_stats.potr.record_failure(detected, latency_epochs);
                if detected {
                    self.repair_backlog += 1;
                    *epoch_repairs += 1;
                    *epoch_repair_peak = (*epoch_repair_peak).max(self.repair_backlog);
                    node.potr_late_epochs = 0;
                }
            } else {
                node.potr_late_epochs = 0;
                epoch_stats.potr.record_success();
            }
        }
    }

    fn sample_partition_regions(&self, rng: &mut ChaCha8Rng) -> Vec<usize> {
        if rng.random::<f64>() >= self.config.partition_probability {
            return Vec::new();
        }
        let mut regions = Vec::new();
        let partitions = rng.random_range(1..=self.config.max_partition_regions);
        while regions.len() < partitions {
            let region = rng.random_range(0..self.config.regions);
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
        regions.sort_unstable();
        regions
    }

    fn node_partitioned(node: &NodeState, partition_regions: &[usize]) -> bool {
        partition_regions.binary_search(&node.region).is_ok()
    }

    fn challenge_fails(
        &self,
        node: &NodeState,
        partition_active: bool,
        rng: &mut ChaCha8Rng,
    ) -> bool {
        if partition_active {
            return true;
        }
        let base = if node.adversarial {
            self.config.adversary_failure_probability
        } else {
            self.config.honest_network_failure_probability
        };
        rng.random::<f64>() < base
    }

    fn potr_fails(&self, node: &NodeState, partition_active: bool, rng: &mut ChaCha8Rng) -> bool {
        if partition_active {
            return true;
        }
        if node.adversarial {
            rng.random::<f64>() < self.config.adversary_failure_probability
        } else {
            rng.random::<f64>() < self.config.honest_network_failure_probability
        }
    }

    fn detect_failure(&self, partitioned: bool, rng: &mut ChaCha8Rng) -> bool {
        partitioned || rng.random::<f64>() < self.config.detection_bias_no_partition
    }

    fn sample_latency_ms(&self, rng: &mut ChaCha8Rng) -> u64 {
        let base = i64::try_from(self.config.challenge_interval.as_millis()).unwrap_or(i64::MAX);
        let jitter = rng.random_range(-50..=75);
        let adjusted = base.saturating_add(jitter).max(1);
        u64::try_from(adjusted).unwrap_or(1)
    }

    fn drain_repairs(&mut self) -> usize {
        let repaired = self
            .repair_backlog
            .min(self.config.repair_capacity_per_epoch);
        self.repair_backlog -= repaired;
        repaired
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(samples: &[u64], target: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut values = samples.to_vec();
    values.sort_unstable();
    let rank = ((values.len() as f64 - 1.0) * target).round() as usize;
    values[rank] as f64
}

/// Run the simulator with the provided configuration and random seed.
pub fn run_simulation(config: SimulationConfig, seed: u64) -> SimulationStats {
    SimulationHarness::new(config).run(seed)
}

fn json_object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_summary_contains_expected_fields() {
        let stats = run_simulation(SimulationConfig::default(), DEFAULT_SEED);
        let value = stats.json_summary();
        assert!(value["config"]["nodes"].as_u64().is_some());
        assert!(
            value["aggregate"]["pdp"]["detection_rate"]
                .as_f64()
                .is_some()
        );
        assert_eq!(
            value["epochs"].as_array().unwrap().len(),
            stats.config.epochs
        );
    }

    #[test]
    fn potr_window_detects_after_threshold() {
        let config = SimulationConfig {
            nodes: 2,
            adversarial: 1,
            regions: 1,
            max_partition_regions: 1,
            partition_probability: 0.0,
            adversary_failure_probability: 1.0,
            honest_network_failure_probability: 1.0,
            potr_late_threshold: 2,
            detection_bias_no_partition: 0.0,
            ..SimulationConfig::default()
        };

        let mut harness = SimulationHarness::new(config);
        let mut rng = ChaCha8Rng::seed_from_u64(DEFAULT_SEED);
        let mut epoch_stats = EpochStats {
            index: 0,
            partition_regions: Vec::new(),
            pdp: ChallengeStats::default(),
            potr: ChallengeStats::default(),
            qos: EpochQosSnapshot::default(),
            detected_failures: 0,
        };
        let mut response_samples = Vec::new();
        let mut epoch_repair_peak = 0;
        let mut epoch_repairs = 0;

        harness.simulate_potr_window(
            &[],
            &mut rng,
            &mut epoch_stats,
            &mut response_samples,
            &mut epoch_repair_peak,
            &mut epoch_repairs,
        );
        assert_eq!(epoch_stats.potr.detected, 0);

        harness.simulate_potr_window(
            &[],
            &mut rng,
            &mut epoch_stats,
            &mut response_samples,
            &mut epoch_repair_peak,
            &mut epoch_repairs,
        );

        assert_eq!(epoch_stats.potr.detected, config.nodes);
        assert_eq!(harness.repair_backlog, config.nodes);
        assert_eq!(epoch_repairs, config.nodes);
        assert_eq!(epoch_repair_peak, harness.repair_backlog);
    }
}
