//! Per-instance Sumeragi consensus series (`sumeragi_*{lane}`).
//!
//! A node runs one Sumeragi instance for the global chain and one for every active lane
//! (`specs/sumeragi_lanes.md`). Every series of an instance carries its `lane` label:
//! [`GLOBAL_LANE`] for the global instance and the decimal lane id for a lane instance. The
//! remaining labels (`reason`, `kind`, `queue`) come from closed enums, so the label set is
//! bounded by the active lane catalog; [`Metrics::remove_sumeragi_instance`] drops the series of
//! a retired lane. The node's Sumeragi driver is the only writer
//! (`iroha_core::sumeragi::metrics`).

use iroha_data_model::sumeragi::SumeragiHaltReason;
use prometheus::{
    Histogram, IntCounter,
    core::{AtomicU64, GenericGauge},
};

use super::Metrics;

/// `lane` label value of the global instance.
pub const GLOBAL_LANE: &str = "global";

/// What a fetch request asks for (`kind` label of `sumeragi_fetch_requests_total`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FetchKind {
    /// A catch-up `SyncRequest` for committed blocks (`specs/sumeragi.md` §6.9 rule 2).
    Sync,
    /// A `FetchPayload` for a wanted block body (`specs/sumeragi.md` §6.9 rule 5).
    Body,
}

impl FetchKind {
    /// Every kind, in label order.
    pub const ALL: [Self; 2] = [Self::Sync, Self::Body];

    /// The `kind` label value.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Body => "body",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Sync => 0,
            Self::Body => 1,
        }
    }
}

/// A bounded driver queue whose drops are counted (`queue` label of `sumeragi_dropped_total`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DropQueue {
    /// Inbound messages dropped by the per-peer ingress bounds (O6).
    Ingress,
    /// Effects held behind a pending safety record and dropped by their bounds (O2, O6).
    Held,
    /// Serving requests dropped by the per-peer serving bounds (§12.2).
    Serve,
}

impl DropQueue {
    /// Every queue, in label order.
    pub const ALL: [Self; 3] = [Self::Ingress, Self::Held, Self::Serve];

    /// The `queue` label value.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ingress => "ingress",
            Self::Held => "held",
            Self::Serve => "serve",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Ingress => 0,
            Self::Held => 1,
            Self::Serve => 2,
        }
    }
}

/// Every halt reason, in `reason` label order of `sumeragi_halted`.
pub const HALT_REASONS: [SumeragiHaltReason; 6] = [
    SumeragiHaltReason::SafetyRecordCorrupt,
    SumeragiHaltReason::SafetyRecordInconsistent,
    SumeragiHaltReason::SafetyViolation(0),
    SumeragiHaltReason::ApplyDiverged(0),
    SumeragiHaltReason::PublicationRecoveryRequired(0),
    SumeragiHaltReason::DriverAnomaly,
];

/// The `reason` label value of `sumeragi_halted` for `reason` (the height it names is not a
/// label).
#[must_use]
pub const fn halt_reason_label(reason: SumeragiHaltReason) -> &'static str {
    match reason {
        SumeragiHaltReason::SafetyRecordCorrupt => "safety_record_corrupt",
        SumeragiHaltReason::SafetyRecordInconsistent => "safety_record_inconsistent",
        SumeragiHaltReason::SafetyViolation(_) => "safety_violation",
        SumeragiHaltReason::ApplyDiverged(_) => "apply_diverged",
        SumeragiHaltReason::PublicationRecoveryRequired(_) => "publication_recovery_required",
        SumeragiHaltReason::DriverAnomaly => "driver_anomaly",
    }
}

const fn halt_reason_index(reason: SumeragiHaltReason) -> usize {
    match reason {
        SumeragiHaltReason::SafetyRecordCorrupt => 0,
        SumeragiHaltReason::SafetyRecordInconsistent => 1,
        SumeragiHaltReason::SafetyViolation(_) => 2,
        SumeragiHaltReason::ApplyDiverged(_) => 3,
        SumeragiHaltReason::PublicationRecoveryRequired(_) => 4,
        SumeragiHaltReason::DriverAnomaly => 5,
    }
}

/// Gauge values of one instance, taken from one status snapshot of its core.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstanceGauges {
    /// Height of the current round.
    pub round_height: u64,
    /// View of the current round.
    pub round_view: u64,
    /// Routing stage of the current round (0, 1 or 2).
    pub round_stage: u8,
    /// Pacemaker level of the current view.
    pub pacemaker_level: u32,
    /// Pacemaker start level of the current height.
    pub pacemaker_start_level: u32,
    /// Retransmission interval `t_retx` in milliseconds.
    pub retransmit_interval_ms: u64,
    /// Committed tip height.
    pub committed_height: u64,
    /// Highest applied height.
    pub applied_height: u64,
    /// Committed, waiting for the next height's configuration.
    pub awaiting_configuration: bool,
    /// A configured key signs at the current height.
    pub signer_present: bool,
    /// The node is not a signing member at its height.
    pub abstaining: bool,
    /// Some key is unanchored.
    pub unanchored: bool,
}

/// The series of one Sumeragi instance, resolved once for its `lane` label.
///
/// Cloning shares the series. The global instance also keeps the `/status` `view_changes`
/// gauge equal to its current view.
#[derive(Clone)]
pub struct InstanceSeries {
    lane: String,
    round_height: GenericGauge<AtomicU64>,
    round_view: GenericGauge<AtomicU64>,
    round_stage: GenericGauge<AtomicU64>,
    pacemaker_level: GenericGauge<AtomicU64>,
    pacemaker_start_level: GenericGauge<AtomicU64>,
    retransmit_interval_ms: GenericGauge<AtomicU64>,
    committed_height: GenericGauge<AtomicU64>,
    applied_height: GenericGauge<AtomicU64>,
    awaiting_configuration: GenericGauge<AtomicU64>,
    signer_present: GenericGauge<AtomicU64>,
    abstaining: GenericGauge<AtomicU64>,
    unanchored: GenericGauge<AtomicU64>,
    halted: [GenericGauge<AtomicU64>; HALT_REASONS.len()],
    commits: IntCounter,
    view_changes: IntCounter,
    timeout_votes: IntCounter,
    fetches: [IntCounter; FetchKind::ALL.len()],
    dropped: [IntCounter; DropQueue::ALL.len()],
    commit_latency_ms: Histogram,
    apply_latency_ms: Histogram,
    global_view: Option<GenericGauge<AtomicU64>>,
}

impl core::fmt::Debug for InstanceSeries {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InstanceSeries")
            .field("lane", &self.lane)
            .finish_non_exhaustive()
    }
}

impl InstanceSeries {
    /// The `lane` label value of the instance.
    #[must_use]
    pub fn lane(&self) -> &str {
        &self.lane
    }

    /// Set the gauges of one status snapshot.
    pub fn set(&self, gauges: &InstanceGauges) {
        self.round_height.set(gauges.round_height);
        self.round_view.set(gauges.round_view);
        self.round_stage.set(u64::from(gauges.round_stage));
        self.pacemaker_level.set(u64::from(gauges.pacemaker_level));
        self.pacemaker_start_level
            .set(u64::from(gauges.pacemaker_start_level));
        self.retransmit_interval_ms
            .set(gauges.retransmit_interval_ms);
        self.committed_height.set(gauges.committed_height);
        self.applied_height.set(gauges.applied_height);
        self.awaiting_configuration
            .set(u64::from(gauges.awaiting_configuration));
        self.signer_present.set(u64::from(gauges.signer_present));
        self.abstaining.set(u64::from(gauges.abstaining));
        self.unanchored.set(u64::from(gauges.unanchored));
        if let Some(view) = &self.global_view {
            view.set(gauges.round_view);
        }
    }

    /// Flag the reason the instance halted (1) and clear every other reason (0); `None`
    /// clears them all.
    pub fn set_halted(&self, reason: Option<SumeragiHaltReason>) {
        let active = reason.map(halt_reason_index);
        for (index, gauge) in self.halted.iter().enumerate() {
            gauge.set(u64::from(active == Some(index)));
        }
    }

    /// Count `count` committed blocks.
    pub fn add_commits(&self, count: u64) {
        self.commits.inc_by(count);
    }

    /// Count `count` views advanced.
    pub fn add_view_changes(&self, count: u64) {
        self.view_changes.inc_by(count);
    }

    /// Count one timeout vote signed by the node.
    pub fn inc_timeout_votes(&self) {
        self.timeout_votes.inc();
    }

    /// Count one fetch request of `kind`.
    pub fn inc_fetch(&self, kind: FetchKind) {
        self.fetches[kind.index()].inc();
    }

    /// Count `count` drops of `queue`.
    pub fn add_dropped(&self, queue: DropQueue, count: u64) {
        self.dropped[queue.index()].inc_by(count);
    }

    /// Observe a proposal-to-CommitQC latency in milliseconds.
    #[allow(clippy::cast_precision_loss)] // bucket resolution far exceeds f64 rounding
    pub fn observe_commit_latency_ms(&self, ms: u64) {
        self.commit_latency_ms.observe(ms as f64);
    }

    /// Observe a CommitQC-to-apply latency in milliseconds.
    #[allow(clippy::cast_precision_loss)] // bucket resolution far exceeds f64 rounding
    pub fn observe_apply_latency_ms(&self, ms: u64) {
        self.apply_latency_ms.observe(ms as f64);
    }
}

impl Metrics {
    /// The series of the Sumeragi instance labelled `lane` ([`GLOBAL_LANE`] or a lane id);
    /// every series of the instance exists (at zero) once this returns.
    #[must_use]
    pub fn sumeragi_instance(&self, lane: &str) -> InstanceSeries {
        let one = [lane];
        let gauge =
            |vec: &prometheus::core::GenericGaugeVec<AtomicU64>| vec.with_label_values(&one);
        InstanceSeries {
            lane: lane.to_owned(),
            round_height: gauge(&self.sumeragi_round_height),
            round_view: gauge(&self.sumeragi_round_view),
            round_stage: gauge(&self.sumeragi_round_stage),
            pacemaker_level: gauge(&self.sumeragi_pacemaker_level),
            pacemaker_start_level: gauge(&self.sumeragi_pacemaker_start_level),
            retransmit_interval_ms: gauge(&self.sumeragi_retransmit_interval_ms),
            committed_height: gauge(&self.sumeragi_committed_height),
            applied_height: gauge(&self.sumeragi_applied_height),
            awaiting_configuration: gauge(&self.sumeragi_awaiting_configuration),
            signer_present: gauge(&self.sumeragi_signer_present),
            abstaining: gauge(&self.sumeragi_abstaining),
            unanchored: gauge(&self.sumeragi_unanchored),
            halted: HALT_REASONS.map(|reason| {
                self.sumeragi_halted
                    .with_label_values(&[lane, halt_reason_label(reason)])
            }),
            commits: self.sumeragi_commits_total.with_label_values(&one),
            view_changes: self.sumeragi_view_changes_total.with_label_values(&one),
            timeout_votes: self.sumeragi_timeout_votes_total.with_label_values(&one),
            fetches: FetchKind::ALL.map(|kind| {
                self.sumeragi_fetch_requests_total
                    .with_label_values(&[lane, kind.label()])
            }),
            dropped: DropQueue::ALL.map(|queue| {
                self.sumeragi_dropped_total
                    .with_label_values(&[lane, queue.label()])
            }),
            commit_latency_ms: self.sumeragi_commit_latency_ms.with_label_values(&one),
            apply_latency_ms: self.sumeragi_apply_latency_ms.with_label_values(&one),
            global_view: (lane == GLOBAL_LANE).then(|| self.view_changes.clone()),
        }
    }

    /// Remove every series of the Sumeragi instance labelled `lane` (its lane retired).
    pub fn remove_sumeragi_instance(&self, lane: &str) {
        let one = [lane];
        for vec in [
            &self.sumeragi_round_height,
            &self.sumeragi_round_view,
            &self.sumeragi_round_stage,
            &self.sumeragi_pacemaker_level,
            &self.sumeragi_pacemaker_start_level,
            &self.sumeragi_retransmit_interval_ms,
            &self.sumeragi_committed_height,
            &self.sumeragi_applied_height,
            &self.sumeragi_awaiting_configuration,
            &self.sumeragi_signer_present,
            &self.sumeragi_abstaining,
            &self.sumeragi_unanchored,
        ] {
            let _ = vec.remove_label_values(&one);
        }
        for reason in HALT_REASONS {
            let _ = self
                .sumeragi_halted
                .remove_label_values(&[lane, halt_reason_label(reason)]);
        }
        for vec in [
            &self.sumeragi_commits_total,
            &self.sumeragi_view_changes_total,
            &self.sumeragi_timeout_votes_total,
        ] {
            let _ = vec.remove_label_values(&one);
        }
        for kind in FetchKind::ALL {
            let _ = self
                .sumeragi_fetch_requests_total
                .remove_label_values(&[lane, kind.label()]);
        }
        for queue in DropQueue::ALL {
            let _ = self
                .sumeragi_dropped_total
                .remove_label_values(&[lane, queue.label()]);
        }
        for vec in [
            &self.sumeragi_commit_latency_ms,
            &self.sumeragi_apply_latency_ms,
        ] {
            let _ = vec.remove_label_values(&one);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exposition lines of the `sumeragi_*` series (without `# HELP`/`# TYPE`).
    fn exposition(metrics: &Metrics) -> Vec<String> {
        metrics
            .try_to_string()
            .expect("render metrics")
            .lines()
            .filter(|line| line.starts_with("sumeragi_"))
            .map(str::to_owned)
            .collect()
    }

    fn has_series(metrics: &Metrics, prefix: &str) -> bool {
        exposition(metrics)
            .iter()
            .any(|line| line.starts_with(prefix))
    }

    #[test]
    fn series_exist_at_zero_and_carry_the_lane_label() {
        let metrics = Metrics::default();
        let series = metrics.sumeragi_instance("7");
        assert_eq!(series.lane(), "7");
        for series_name in [
            "sumeragi_round_height{lane=\"7\"} 0",
            "sumeragi_round_view{lane=\"7\"} 0",
            "sumeragi_round_stage{lane=\"7\"} 0",
            "sumeragi_pacemaker_level{lane=\"7\"} 0",
            "sumeragi_pacemaker_start_level{lane=\"7\"} 0",
            "sumeragi_retransmit_interval_ms{lane=\"7\"} 0",
            "sumeragi_committed_height{lane=\"7\"} 0",
            "sumeragi_applied_height{lane=\"7\"} 0",
            "sumeragi_awaiting_configuration{lane=\"7\"} 0",
            "sumeragi_signer_present{lane=\"7\"} 0",
            "sumeragi_abstaining{lane=\"7\"} 0",
            "sumeragi_unanchored{lane=\"7\"} 0",
            "sumeragi_commits_total{lane=\"7\"} 0",
            "sumeragi_view_changes_total{lane=\"7\"} 0",
            "sumeragi_timeout_votes_total{lane=\"7\"} 0",
            "sumeragi_commit_latency_ms_count{lane=\"7\"} 0",
            "sumeragi_apply_latency_ms_count{lane=\"7\"} 0",
        ] {
            assert!(
                exposition(&metrics).iter().any(|line| line == series_name),
                "{series_name}"
            );
        }
        for reason in HALT_REASONS {
            let line = format!(
                "sumeragi_halted{{lane=\"7\",reason=\"{}\"}} 0",
                halt_reason_label(reason)
            );
            assert!(exposition(&metrics).contains(&line), "{line}");
        }
        for kind in FetchKind::ALL {
            let line = format!(
                "sumeragi_fetch_requests_total{{kind=\"{}\",lane=\"7\"}} 0",
                kind.label()
            );
            assert!(exposition(&metrics).contains(&line), "{line}");
        }
        for queue in DropQueue::ALL {
            let line = format!(
                "sumeragi_dropped_total{{lane=\"7\",queue=\"{}\"}} 0",
                queue.label()
            );
            assert!(exposition(&metrics).contains(&line), "{line}");
        }
    }

    #[test]
    fn gauges_counters_and_histograms_record_values() {
        let metrics = Metrics::default();
        let series = metrics.sumeragi_instance("3");
        series.set(&InstanceGauges {
            round_height: 12,
            round_view: 2,
            round_stage: 1,
            pacemaker_level: 3,
            pacemaker_start_level: 1,
            retransmit_interval_ms: 400,
            committed_height: 11,
            applied_height: 10,
            awaiting_configuration: true,
            signer_present: true,
            abstaining: false,
            unanchored: true,
        });
        series.add_commits(4);
        series.add_view_changes(2);
        series.inc_timeout_votes();
        series.inc_fetch(FetchKind::Sync);
        series.inc_fetch(FetchKind::Body);
        series.inc_fetch(FetchKind::Body);
        series.add_dropped(DropQueue::Held, 5);
        series.observe_commit_latency_ms(900);
        series.observe_apply_latency_ms(40);
        let lane = ["3"];
        for (vec, expected) in [
            (&metrics.sumeragi_round_height, 12),
            (&metrics.sumeragi_round_view, 2),
            (&metrics.sumeragi_round_stage, 1),
            (&metrics.sumeragi_pacemaker_level, 3),
            (&metrics.sumeragi_pacemaker_start_level, 1),
            (&metrics.sumeragi_retransmit_interval_ms, 400),
            (&metrics.sumeragi_committed_height, 11),
            (&metrics.sumeragi_applied_height, 10),
            (&metrics.sumeragi_awaiting_configuration, 1),
            (&metrics.sumeragi_signer_present, 1),
            (&metrics.sumeragi_abstaining, 0),
            (&metrics.sumeragi_unanchored, 1),
        ] {
            assert_eq!(vec.with_label_values(&lane).get(), expected);
        }
        assert_eq!(
            metrics
                .sumeragi_commits_total
                .with_label_values(&lane)
                .get(),
            4
        );
        assert_eq!(
            metrics
                .sumeragi_view_changes_total
                .with_label_values(&lane)
                .get(),
            2
        );
        assert_eq!(
            metrics
                .sumeragi_timeout_votes_total
                .with_label_values(&lane)
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_fetch_requests_total
                .with_label_values(&["3", "sync"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_fetch_requests_total
                .with_label_values(&["3", "body"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .sumeragi_dropped_total
                .with_label_values(&["3", "held"])
                .get(),
            5
        );
        let commit = metrics.sumeragi_commit_latency_ms.with_label_values(&lane);
        assert_eq!(commit.get_sample_count(), 1);
        assert!((commit.get_sample_sum() - 900.0).abs() < f64::EPSILON);
        let apply = metrics.sumeragi_apply_latency_ms.with_label_values(&lane);
        assert_eq!(apply.get_sample_count(), 1);
        assert!((apply.get_sample_sum() - 40.0).abs() < f64::EPSILON);
        assert_eq!(
            metrics.view_changes.get(),
            0,
            "only the global view is /status"
        );
    }

    #[test]
    fn halted_flags_exactly_one_reason() {
        let metrics = Metrics::default();
        let series = metrics.sumeragi_instance(GLOBAL_LANE);
        let flagged = || -> Vec<&'static str> {
            HALT_REASONS
                .into_iter()
                .map(halt_reason_label)
                .filter(|label| {
                    metrics
                        .sumeragi_halted
                        .with_label_values(&[GLOBAL_LANE, label])
                        .get()
                        == 1
                })
                .collect()
        };
        series.set_halted(Some(SumeragiHaltReason::ApplyDiverged(9)));
        assert_eq!(flagged(), ["apply_diverged"]);
        series.set_halted(Some(SumeragiHaltReason::DriverAnomaly));
        assert_eq!(flagged(), ["driver_anomaly"]);
        series.set_halted(None);
        assert!(flagged().is_empty());
    }

    #[test]
    fn halt_reason_labels_are_distinct_and_cover_every_reason() {
        let labels: std::collections::BTreeSet<&str> =
            HALT_REASONS.into_iter().map(halt_reason_label).collect();
        assert_eq!(labels.len(), HALT_REASONS.len());
        for (index, reason) in HALT_REASONS.into_iter().enumerate() {
            assert_eq!(halt_reason_index(reason), index);
        }
        assert_eq!(
            halt_reason_label(SumeragiHaltReason::SafetyViolation(42)),
            "safety_violation",
            "the height a reason names is not a label"
        );
    }

    #[test]
    fn global_view_is_the_status_view_changes_gauge() {
        let metrics = Metrics::default();
        let series = metrics.sumeragi_instance(GLOBAL_LANE);
        series.set(&InstanceGauges {
            round_view: 5,
            ..InstanceGauges::default()
        });
        assert_eq!(metrics.view_changes.get(), 5);
        let lane = metrics.sumeragi_instance("1");
        lane.set(&InstanceGauges {
            round_view: 9,
            ..InstanceGauges::default()
        });
        assert_eq!(metrics.view_changes.get(), 5);
    }

    /// The exported `sumeragi_*` families of `metrics` (from their `# TYPE` lines).
    fn families(metrics: &Metrics) -> std::collections::BTreeSet<String> {
        metrics
            .try_to_string()
            .expect("render metrics")
            .lines()
            .filter_map(|line| line.strip_prefix("# TYPE "))
            .filter_map(|line| line.split(' ').next())
            .filter(|family| family.starts_with("sumeragi_"))
            .map(str::to_owned)
            .collect()
    }

    /// Every registered `sumeragi_*` family has a live writer: it is either one of the
    /// transaction-queue gauges (always exported, written by the node's queue telemetry) or a
    /// family of [`InstanceSeries`] (written by the node's Sumeragi driver). The node crate
    /// checks that those writers move every one of them off zero.
    #[test]
    fn registered_sumeragi_families_are_the_queue_gauges_and_the_instance_series() {
        let registered: std::collections::BTreeSet<String> = include_str!("catalog_v2.tsv")
            .lines()
            .filter(|row| row.ends_with("\tregistered"))
            .filter_map(|row| row.split('\t').nth(1))
            .filter(|name| name.starts_with("sumeragi_"))
            .map(str::to_owned)
            .collect();
        let metrics = Metrics::default();
        let always = families(&metrics);
        assert!(
            always
                .iter()
                .all(|family| family.starts_with("sumeragi_tx_queue_")),
            "{always:?}"
        );
        let _series = metrics.sumeragi_instance(GLOBAL_LANE);
        assert_eq!(families(&metrics), registered);
    }

    #[test]
    fn removing_an_instance_drops_only_its_series() {
        let metrics = Metrics::default();
        let global = metrics.sumeragi_instance(GLOBAL_LANE);
        let lane = metrics.sumeragi_instance("4");
        global.add_commits(1);
        lane.add_commits(2);
        lane.set_halted(Some(SumeragiHaltReason::DriverAnomaly));
        lane.observe_apply_latency_ms(3);
        metrics.remove_sumeragi_instance("4");
        assert!(
            exposition(&metrics)
                .iter()
                .all(|line| !line.contains("lane=\"4\"")),
            "{:?}",
            exposition(&metrics)
        );
        assert!(has_series(
            &metrics,
            "sumeragi_commits_total{lane=\"global\"} 1"
        ));
        assert!(has_series(
            &metrics,
            "sumeragi_halted{lane=\"global\",reason="
        ));
    }
}
