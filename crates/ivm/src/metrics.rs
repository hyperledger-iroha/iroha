//! Process-wide IVM performance metrics.
//!
//! IVM owns the Prometheus metrics for its standalone Merkle roots, Merkle
//! cache updates and memory commits, so the VM does not depend on the node
//! telemetry stack. They are local diagnostics: they never enter gas, state,
//! receipts or commitments. A node exports them by registering [`collector`]
//! with its metrics registry; Core does this when node telemetry starts.

use std::{sync::LazyLock, time::Duration};

use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, Opts,
    core::{Collector, Desc},
    exponential_buckets, proto,
};

/// Backend that computed one standalone Merkle root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MerkleRootBackend {
    /// Metal or CUDA computed the root.
    #[cfg(any(test, target_os = "macos", feature = "cuda"))]
    Gpu,
    /// The canonical CPU tree computed the root.
    Cpu,
}

/// How one memory commit refreshed its Merkle leaves (the exported `path` label).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemoryCommitPath {
    /// Only the dirty leaves were rehashed.
    Incremental,
    /// Hardware recomputed every leaf.
    Accel,
    /// The CPU recomputed every leaf in parallel.
    FullRebuild,
}

impl MemoryCommitPath {
    /// Exported `path` label value.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::Accel => "accel",
            Self::FullRebuild => "full_rebuild",
        }
    }
}

struct IvmMetrics {
    merkle_root_gpu_total: IntCounter,
    merkle_root_cpu_total: IntCounter,
    memory_commit_ms: HistogramVec,
    memory_commit_dirty_chunks: HistogramVec,
    merkle_rebuild_total: IntCounter,
    merkle_incremental_leaf_updates_total: IntCounter,
}

impl IvmMetrics {
    fn new() -> Self {
        let counter = |name: &str, help: &str| {
            IntCounter::with_opts(Opts::new(name, help)).expect("valid IVM counter descriptor")
        };
        let histogram = |name: &str, help: &str, buckets: Vec<f64>| {
            HistogramVec::new(HistogramOpts::new(name, help).buckets(buckets), &["path"])
                .expect("valid IVM histogram descriptor")
        };
        Self {
            merkle_root_gpu_total: counter(
                "merkle_root_gpu_total",
                "Merkle root computations using GPU acceleration (cumulative)",
            ),
            merkle_root_cpu_total: counter(
                "merkle_root_cpu_total",
                "Merkle root computations using CPU (cumulative)",
            ),
            memory_commit_ms: histogram(
                "ivm_memory_commit_ms",
                "IVM memory commit duration in milliseconds by commit path",
                exponential_buckets(0.1, 2.0, 16).expect("valid commit duration buckets"),
            ),
            memory_commit_dirty_chunks: histogram(
                "ivm_memory_commit_dirty_chunks",
                "IVM memory commit dirty chunk count by commit path",
                exponential_buckets(1.0, 2.0, 20).expect("valid dirty chunk buckets"),
            ),
            merkle_rebuild_total: counter(
                "ivm_merkle_rebuild_total",
                "IVM completed in-place canonical Merkle node refreshes (excluding construction and copies)",
            ),
            merkle_incremental_leaf_updates_total: counter(
                "ivm_merkle_incremental_leaf_updates_total",
                "IVM Merkle cache incremental leaf updates",
            ),
        }
    }

    fn collectors(&self) -> [&dyn Collector; 6] {
        [
            &self.merkle_root_gpu_total,
            &self.merkle_root_cpu_total,
            &self.memory_commit_ms,
            &self.memory_commit_dirty_chunks,
            &self.merkle_rebuild_total,
            &self.merkle_incremental_leaf_updates_total,
        ]
    }
}

static METRICS: LazyLock<IvmMetrics> = LazyLock::new(IvmMetrics::new);

/// Count one standalone Merkle root computed by `backend`.
pub(crate) fn record_merkle_root(backend: MerkleRootBackend) {
    match backend {
        #[cfg(any(test, target_os = "macos", feature = "cuda"))]
        MerkleRootBackend::Gpu => METRICS.merkle_root_gpu_total.inc(),
        MerkleRootBackend::Cpu => METRICS.merkle_root_cpu_total.inc(),
    }
}

/// Count one completed in-place canonical node refresh.
pub(crate) fn record_merkle_rebuild() {
    METRICS.merkle_rebuild_total.inc();
}

/// Count leaves updated along incremental canonical paths.
pub(crate) fn record_merkle_incremental_leaf_updates(leaves: u64) {
    METRICS.merkle_incremental_leaf_updates_total.inc_by(leaves);
}

/// Record one memory commit's duration and the dirty chunk count it consumed.
pub(crate) fn record_memory_commit(path: MemoryCommitPath, elapsed: Duration, dirty_chunks: usize) {
    let labels = [path.label()];
    METRICS
        .memory_commit_ms
        .with_label_values(&labels)
        .observe(elapsed.as_secs_f64() * 1_000.0);
    METRICS
        .memory_commit_dirty_chunks
        .with_label_values(&labels)
        .observe(dirty_chunks as f64);
}

/// Cumulative completed canonical-node refreshes and incremental leaf updates.
#[cfg(test)]
pub(crate) fn merkle_update_totals() -> (u64, u64) {
    (
        METRICS.merkle_rebuild_total.get(),
        METRICS.merkle_incremental_leaf_updates_total.get(),
    )
}

/// Prometheus collector over the process-wide IVM metrics.
///
/// Every collector reads the same values. Register one in each registry that
/// should expose them; a second registration in the same registry fails with
/// [`prometheus::Error::AlreadyReg`].
#[derive(Clone, Copy)]
pub struct IvmMetricsCollector {
    metrics: &'static IvmMetrics,
}

impl std::fmt::Debug for IvmMetricsCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IvmMetricsCollector")
            .finish_non_exhaustive()
    }
}

impl Collector for IvmMetricsCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.metrics
            .collectors()
            .into_iter()
            .flat_map(|metric| metric.desc())
            .collect()
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        self.metrics
            .collectors()
            .into_iter()
            .flat_map(|metric| metric.collect())
            .collect()
    }
}

/// Collector for the process-wide IVM metrics, for registration with a node registry.
#[must_use]
pub fn collector() -> IvmMetricsCollector {
    IvmMetricsCollector {
        metrics: LazyLock::force(&METRICS),
    }
}

#[cfg(test)]
mod tests {
    use prometheus::Registry;

    use super::*;

    #[test]
    fn collector_keeps_the_exported_metric_families() {
        let families: Vec<(String, String, Vec<String>)> = collector()
            .desc()
            .into_iter()
            .map(|desc| {
                (
                    desc.fq_name.clone(),
                    desc.help.clone(),
                    desc.variable_labels.clone(),
                )
            })
            .collect();
        let expected = [
            (
                "merkle_root_gpu_total",
                "Merkle root computations using GPU acceleration (cumulative)",
                None,
            ),
            (
                "merkle_root_cpu_total",
                "Merkle root computations using CPU (cumulative)",
                None,
            ),
            (
                "ivm_memory_commit_ms",
                "IVM memory commit duration in milliseconds by commit path",
                Some("path"),
            ),
            (
                "ivm_memory_commit_dirty_chunks",
                "IVM memory commit dirty chunk count by commit path",
                Some("path"),
            ),
            (
                "ivm_merkle_rebuild_total",
                "IVM completed in-place canonical Merkle node refreshes (excluding construction and copies)",
                None,
            ),
            (
                "ivm_merkle_incremental_leaf_updates_total",
                "IVM Merkle cache incremental leaf updates",
                None,
            ),
        ]
        .map(|(name, help, label)| {
            (
                name.to_owned(),
                help.to_owned(),
                label.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            )
        });
        assert_eq!(families, expected);
    }

    #[test]
    fn memory_commit_path_labels_are_stable() {
        assert_eq!(MemoryCommitPath::Incremental.label(), "incremental");
        assert_eq!(MemoryCommitPath::Accel.label(), "accel");
        assert_eq!(MemoryCommitPath::FullRebuild.label(), "full_rebuild");
    }

    #[test]
    fn merkle_events_increment_their_counters() {
        // Other tests record concurrently into the same process-wide counters.
        let (rebuilds, leaves) = merkle_update_totals();
        let gpu = METRICS.merkle_root_gpu_total.get();
        let cpu = METRICS.merkle_root_cpu_total.get();
        record_merkle_rebuild();
        record_merkle_incremental_leaf_updates(5);
        record_merkle_root(MerkleRootBackend::Gpu);
        record_merkle_root(MerkleRootBackend::Cpu);
        let (rebuilds_after, leaves_after) = merkle_update_totals();
        assert!(rebuilds_after > rebuilds);
        assert!(leaves_after >= leaves + 5);
        assert!(METRICS.merkle_root_gpu_total.get() > gpu);
        assert!(METRICS.merkle_root_cpu_total.get() > cpu);
    }

    #[test]
    fn registered_collector_exposes_memory_commits_with_fixed_buckets() {
        let registry = Registry::new();
        registry
            .register(Box::new(collector()))
            .expect("first IVM collector registers");
        assert!(matches!(
            registry.register(Box::new(collector())),
            Err(prometheus::Error::AlreadyReg)
        ));
        record_memory_commit(MemoryCommitPath::FullRebuild, Duration::from_millis(3), 7);
        let families = registry.gather();
        for (name, expected_bounds) in [
            ("ivm_memory_commit_ms", exponential_buckets(0.1, 2.0, 16)),
            (
                "ivm_memory_commit_dirty_chunks",
                exponential_buckets(1.0, 2.0, 20),
            ),
        ] {
            let family = families
                .iter()
                .find(|family| family.name() == name)
                .unwrap_or_else(|| panic!("`{name}` is exported after a commit"));
            let histogram = family
                .get_metric()
                .iter()
                .find(|metric| {
                    metric
                        .get_label()
                        .iter()
                        .any(|pair| pair.name() == "path" && pair.value() == "full_rebuild")
                })
                .unwrap_or_else(|| panic!("`{name}` has a full_rebuild series"))
                .get_histogram();
            assert!(histogram.get_sample_count() >= 1);
            let bounds: Vec<f64> = histogram
                .get_bucket()
                .iter()
                .map(|bucket| bucket.upper_bound())
                .collect();
            assert_eq!(bounds, expected_bounds.expect("valid buckets"));
        }
        for counter in [
            "merkle_root_gpu_total",
            "merkle_root_cpu_total",
            "ivm_merkle_rebuild_total",
            "ivm_merkle_incremental_leaf_updates_total",
        ] {
            assert!(
                families.iter().any(|family| family.name() == counter),
                "`{counter}` is always exported"
            );
        }
    }
}
