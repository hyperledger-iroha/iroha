//! Whole original transform invocations under their explicit diagnostic parent.
//!
//! Families describe public canonical relations. Existing backend work counters
//! attach to these coordinator scopes; no worker time or private data is added.
//! The finished-record validator is separate from the recorder's general policy.

use super::{MEASUREMENT_ROOT_V1, PhaseV1, ReceiptV1};
use iroha_measurement::{PhaseGuard, PhaseParent, RecorderHealth};

/// Number of closed public families, including genuinely mixed batches.
pub(crate) const FAMILY_COUNT: usize = 13;
/// Challenge phase of the original joined commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum KindV1 {
    /// Challenge-independent columns.
    Base,
    /// Challenge-dependent columns.
    Aux,
}
/// Existing relation identities; coordinates and instance ordinals are not labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum FamilyV1 {
    /// Original ByteMemory relation family.
    ByteMemory,
    /// Original MixedRelations relation family.
    MixedRelations,
    /// Original P256Arithmetic relation family.
    P256Arithmetic,
    /// Original P256BindingSink relation family.
    P256BindingSink,
    /// Original P256Reduction relation family.
    P256Reduction,
    /// Original P256ScalarBitBus relation family.
    P256ScalarBitBus,
    /// Original P256ValueBus relation family.
    P256ValueBus,
    /// Original P256WalletLowS relation family.
    P256WalletLowS,
    /// Original P256WindowBatch relation family.
    P256WindowBatch,
    /// Original Projection relation family.
    Projection,
    /// Original Rfc5280 relation family.
    Rfc5280,
    /// Original Sha256CallBus relation family.
    Sha256CallBus,
    /// Original StrictDer relation family.
    StrictDer,
}
/// Complete closed family inventory, ordered only for bounded local accounting.
pub(crate) const FAMILIES: [FamilyV1; FAMILY_COUNT] = [
    FamilyV1::ByteMemory,
    FamilyV1::MixedRelations,
    FamilyV1::P256Arithmetic,
    FamilyV1::P256BindingSink,
    FamilyV1::P256Reduction,
    FamilyV1::P256ScalarBitBus,
    FamilyV1::P256ValueBus,
    FamilyV1::P256WalletLowS,
    FamilyV1::P256WindowBatch,
    FamilyV1::Projection,
    FamilyV1::Rfc5280,
    FamilyV1::Sha256CallBus,
    FamilyV1::StrictDer,
];
const LABELS: [[&str; FAMILY_COUNT]; 2] = [
    [
        "TransformBaseByteMemory",
        "TransformBaseMixedRelations",
        "TransformBaseP256Arithmetic",
        "TransformBaseP256BindingSink",
        "TransformBaseP256Reduction",
        "TransformBaseP256ScalarBitBus",
        "TransformBaseP256ValueBus",
        "TransformBaseP256WalletLowS",
        "TransformBaseP256WindowBatch",
        "TransformBaseProjection",
        "TransformBaseRfc5280",
        "TransformBaseSha256CallBus",
        "TransformBaseStrictDer",
    ],
    [
        "TransformAuxByteMemory",
        "TransformAuxMixedRelations",
        "TransformAuxP256Arithmetic",
        "TransformAuxP256BindingSink",
        "TransformAuxP256Reduction",
        "TransformAuxP256ScalarBitBus",
        "TransformAuxP256ValueBus",
        "TransformAuxP256WalletLowS",
        "TransformAuxP256WindowBatch",
        "TransformAuxProjection",
        "TransformAuxRfc5280",
        "TransformAuxSha256CallBus",
        "TransformAuxStrictDer",
    ],
];
const INVALID_LABEL: &str = "TransformAttributionInvalid";
impl FamilyV1 {
    /// Static label for this public family and original challenge phase.
    pub(crate) const fn label(self, kind: KindV1) -> &'static str {
        LABELS[kind as usize][self as usize]
    }
}
/// Public metadata could not name exactly one original batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvalidAttributionV1;
/// Registry-derived expected successful calls and columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TallyV1 {
    /// Whole original evaluator invocations.
    pub(crate) calls: u64,
    /// Original columns, independent of backend submission count.
    pub(crate) columns: u64,
}
/// Expected Base/Aux partition, reconstructed from the canonical registry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExpectedV1(pub(crate) [[TallyV1; FAMILY_COUNT]; 2]);
impl ExpectedV1 {
    /// Checked sum for one original challenge phase.
    pub(crate) fn total(&self, kind: KindV1) -> Option<TallyV1> {
        self.0[kind as usize]
            .iter()
            .try_fold(TallyV1::default(), |sum, row| {
                Some(TallyV1 {
                    calls: sum.calls.checked_add(row.calls)?,
                    columns: sum.columns.checked_add(row.columns)?,
                })
            })
    }
}
/// Exact invoking timer's parent; inactivity never adopts another observation.
#[derive(Clone, Default)]
pub(crate) struct ContextV1(Option<PhaseParent>);
impl ContextV1 {
    /// Delegate only this supplied original parent.
    pub(crate) fn from_parent(parent: Option<PhaseParent>) -> Self {
        Self(parent)
    }
    /// Open one whole-batch scope, including a visible invalid-attribution marker.
    pub(crate) fn enter(
        &self,
        kind: KindV1,
        family: Result<FamilyV1, InvalidAttributionV1>,
    ) -> GuardV1 {
        let valid = family.is_ok();
        let label = family.map_or(INVALID_LABEL, |family| family.label(kind));
        GuardV1 {
            guard: self.0.as_ref().map(|parent| parent.enter(label)),
            valid,
        }
    }
}
/// Opening-thread guard, dropped before its original transform timer.
pub(crate) struct GuardV1 {
    guard: Option<PhaseGuard>,
    valid: bool,
}
impl GuardV1 {
    /// Complete real successful evaluation; invalid attribution stays interrupted.
    pub(crate) fn complete(self) {
        if self.valid {
            if let Some(guard) = self.guard {
                guard.complete();
            }
        }
    }
}
/// Closed diagnostic coverage refusal, containing no producer error or private data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoverageErrorV1 {
    /// The shared record is missing or unhealthy.
    Record,
    /// An invalid public coordinate was recorded.
    InvalidAttribution,
    /// The original parent or family topology is wrong.
    Topology,
    /// A call, outcome or column total differs from the canonical partition.
    Census,
    /// Backend work is missing, duplicated, outside its family, or inconsistent.
    Work,
}

impl ReceiptV1 {
    /// Refuse incomplete attribution after the original proof and record are retained.
    ///
    /// This does not change the recorder's outcome or the producer's Result. An
    /// invalid child can coexist with producer success; the diagnostic must still
    /// fail this check before it can report successful qualification.
    pub(crate) fn validate_transform_family_coverage_v1(
        &self,
        expected: &ExpectedV1,
    ) -> Result<(), CoverageErrorV1> {
        use CoverageErrorV1 as Error;
        let record = self.measurement_v1().ok_or(Error::Record)?;
        if record.recorder != RecorderHealth::default()
            || record.outcome != iroha_measurement::RunOutcome::Succeeded
        {
            return Err(Error::Record);
        }
        let nodes = &record.phase_tree.nodes;
        if nodes
            .first()
            .is_none_or(|node| node.parent.is_some() || node.label != MEASUREMENT_ROOT_V1)
        {
            return Err(Error::Topology);
        }
        if nodes.iter().any(|node| node.label == INVALID_LABEL) {
            return Err(Error::InvalidAttribution);
        }
        let unique = |parent: usize, label: &str| -> Result<usize, Error> {
            let mut matching = nodes.iter().enumerate().filter(|(_, node)| {
                node.parent.and_then(|p| usize::try_from(p).ok()) == Some(parent)
                    && node.label == label
            });
            let (index, _) = matching.next().ok_or(Error::Topology)?;
            if matching.next().is_some() {
                return Err(Error::Topology);
            }
            Ok(index)
        };
        let success = |index: usize, calls: u64| -> Result<(), Error> {
            let node = &nodes[index];
            if node.calls != calls
                || node.completed != calls
                || node.interrupted != 0
                || node.unwound != 0
                || node.truncated != 0
            {
                return Err(Error::Census);
            }
            Ok(())
        };
        let base = expected.total(KindV1::Base).ok_or(Error::Census)?;
        let aux = expected.total(KindV1::Aux).ok_or(Error::Census)?;
        let calls = base.calls.checked_add(aux.calls).ok_or(Error::Census)?;
        let columns = base
            .columns
            .checked_add(aux.columns)
            .and_then(|sum| sum.checked_mul(2))
            .ok_or(Error::Census)?;
        let base_parent = unique(unique(0, "BaseSampleAndCommit")?, "InitialJoinedTransform")?;
        let aux_parent = unique(unique(0, "AuxSampleAndCommit")?, "InitialJoinedTransform")?;
        let query_parent = unique(unique(0, "QueryOpenings")?, "QueryJoinedTransform")?;
        success(base_parent, base.calls)?;
        success(aux_parent, aux.calls)?;
        success(query_parent, calls)?;
        let mut owned = [None; FAMILY_COUNT * 4];
        let mut backend_calls = [0_u64; 2];
        let mut backend_columns = [0_u64; 2];
        for (flow, (parent, kind)) in [
            (base_parent, KindV1::Base),
            (aux_parent, KindV1::Aux),
            (query_parent, KindV1::Base),
            (query_parent, KindV1::Aux),
        ]
        .into_iter()
        .enumerate()
        {
            for family in FAMILIES {
                let tally = expected.0[kind as usize][family as usize];
                if tally.calls == 0 || tally.columns == 0 {
                    return Err(Error::Census);
                }
                let index = unique(parent, family.label(kind))?;
                owned[flow * FAMILY_COUNT + family as usize] = Some(index);
                success(index, tally.calls)?;
                let mut total = 0_u64;
                for (backend, label) in ["transform_cpu_columns", "transform_metal_columns"]
                    .into_iter()
                    .enumerate()
                {
                    let mut matching = record.work_counters.entries.iter().filter(|entry| {
                        usize::try_from(entry.phase).ok() == Some(index) && entry.label == label
                    });
                    if let Some(entry) = matching.next() {
                        if matching.next().is_some()
                            || entry.count == 0
                            || entry.min_units == 0
                            || entry.min_units > entry.max_units
                            || entry.max_units > 8
                            || entry
                                .count
                                .checked_mul(entry.min_units)
                                .is_none_or(|minimum| entry.total_units < minimum)
                            || entry
                                .count
                                .checked_mul(entry.max_units)
                                .is_none_or(|maximum| entry.total_units > maximum)
                        {
                            return Err(Error::Work);
                        }
                        total = total.checked_add(entry.total_units).ok_or(Error::Work)?;
                        backend_calls[backend] = backend_calls[backend]
                            .checked_add(entry.count)
                            .ok_or(Error::Work)?;
                        backend_columns[backend] = backend_columns[backend]
                            .checked_add(entry.total_units)
                            .ok_or(Error::Work)?;
                    }
                }
                if total != tally.columns {
                    return Err(Error::Work);
                }
            }
        }
        // No family or completion counter can hide under a different parent.
        for (index, node) in nodes.iter().enumerate() {
            if node
                .parent
                .and_then(|parent| usize::try_from(parent).ok())
                .is_some_and(|parent| [base_parent, aux_parent, query_parent].contains(&parent))
                && !owned.contains(&Some(index))
            {
                return Err(Error::Topology);
            }
            if node.label.starts_with("Transform") && !owned.contains(&Some(index)) {
                return Err(Error::Topology);
            }
            if matches!(
                node.label.as_str(),
                "InitialJoinedTransform" | "QueryJoinedTransform"
            ) && ![base_parent, aux_parent, query_parent].contains(&index)
            {
                return Err(Error::Topology);
            }
        }
        for entry in &record.work_counters.entries {
            if entry.label == "transform_failed_calls" {
                return Err(Error::Work);
            }
            if matches!(
                entry.label.as_str(),
                "transform_cpu_columns" | "transform_metal_columns"
            ) && !owned.contains(&usize::try_from(entry.phase).ok())
            {
                return Err(Error::Work);
            }
        }
        if backend_calls != [self.cpu_calls, self.metal_calls]
            || backend_columns != [self.cpu_columns, self.metal_columns]
            || self.cpu_columns.checked_add(self.metal_columns) != Some(columns)
            || self.failures != 0
        {
            return Err(Error::Work);
        }
        for phase in [
            PhaseV1::InitialJoinedTransform,
            PhaseV1::QueryJoinedTransform,
        ] {
            let flat = self.phases[phase as usize];
            if flat.calls != calls
                || flat.completed != calls
                || flat.interrupted != 0
                || flat.unwound != 0
            {
                return Err(Error::Census);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::prover_observation::{
        ObservationV1, PhaseTimerV1, completed_transform_v1,
    };

    fn finished_partition(split: bool) -> (ReceiptV1, ExpectedV1) {
        let observation = ObservationV1::begin_v1();
        for (phase, kinds) in [
            (PhaseV1::BaseSampleAndCommit, &[KindV1::Base][..]),
            (PhaseV1::AuxSampleAndCommit, &[KindV1::Aux][..]),
            (PhaseV1::QueryOpenings, &[KindV1::Base, KindV1::Aux][..]),
        ] {
            let outer = PhaseTimerV1::start_v1(phase);
            for &kind in kinds {
                for family in FAMILIES {
                    let timer = PhaseTimerV1::start_v1(if kinds.len() == 2 {
                        PhaseV1::QueryJoinedTransform
                    } else {
                        PhaseV1::InitialJoinedTransform
                    });
                    let child = timer.transform_context_v1().enter(kind, Ok(family));
                    if split {
                        completed_transform_v1(false, 2);
                        completed_transform_v1(true, 6);
                    } else {
                        completed_transform_v1(false, 8);
                    }
                    child.complete();
                    timer.complete_v1();
                }
            }
            outer.complete_v1();
        }
        (
            observation.finish_v1(),
            ExpectedV1(
                [[TallyV1 {
                    calls: 1,
                    columns: 8,
                }; FAMILY_COUNT]; 2],
            ),
        )
    }

    #[test]
    fn successful_partition_separates_semantic_calls_from_backend_split_calls() {
        for split in [false, true] {
            let (receipt, expected) = finished_partition(split);
            assert_eq!(
                receipt.validate_transform_family_coverage_v1(&expected),
                Ok(())
            );
            assert_eq!(receipt.cpu_calls, 52);
            assert_eq!(receipt.metal_calls, if split { 52 } else { 0 });
            assert_eq!(receipt.cpu_columns + receipt.metal_columns, 416);
            assert_eq!(receipt.phases.len(), 75);
            let record = receipt.measurement_v1().unwrap();
            let children = record
                .phase_tree
                .nodes
                .iter()
                .filter(|node| node.label.starts_with("Transform"));
            assert_eq!(children.clone().count(), 52);
            assert_eq!(children.map(|node| node.calls).sum::<u64>(), 52);
        }
    }

    #[test]
    fn finished_coverage_refuses_mutated_nodes_counters_and_invalid_success() {
        let (mut receipt, expected) = finished_partition(true);
        let original = receipt.measurement.as_ref().unwrap().clone();
        let semantic = original
            .phase_tree
            .nodes
            .iter()
            .position(|node| node.label == FamilyV1::MixedRelations.label(KindV1::Base))
            .unwrap();
        for mutation in 0..15 {
            receipt.measurement = Some(original.clone());
            let record = receipt.measurement.as_mut().unwrap();
            match mutation {
                0 => {
                    record.phase_tree.nodes.pop();
                }
                1 => {
                    record
                        .phase_tree
                        .nodes
                        .push(record.phase_tree.nodes[semantic].clone());
                }
                2 => {
                    record.phase_tree.nodes[semantic].parent = Some(0);
                }
                3 => {
                    record.phase_tree.nodes[semantic].calls += 1;
                    record.phase_tree.nodes[semantic].completed += 1;
                }
                4 => {
                    record.work_counters.entries[0].total_units += 1;
                }
                5 => {
                    record.work_counters.entries[0].phase = 0;
                }
                6 => {
                    record.phase_tree.nodes[semantic].label = INVALID_LABEL.into();
                }
                7 => {
                    record.phase_tree.nodes[semantic].label =
                        FamilyV1::P256WindowBatch.label(KindV1::Base).into();
                }
                8 => {
                    record.recorder.stale_parent_events = 1;
                }
                9 => {
                    let mut extra = record.work_counters.entries[0].clone();
                    extra.label = "transform_failed_calls".into();
                    record.work_counters.entries.push(extra);
                }
                10 => {
                    record.work_counters.entries[0].count += 1;
                }
                11 => {
                    record.phase_tree.nodes[semantic].completed -= 1;
                    record.phase_tree.nodes[semantic].interrupted += 1;
                }
                12 => {
                    let mut extra = record.phase_tree.nodes[semantic].clone();
                    extra.label = "UnregisteredOperation".into();
                    record.phase_tree.nodes.push(extra);
                }
                13 => {
                    record.phase_tree.nodes[semantic].label = "UnregisteredOperation".into();
                }
                14 => {
                    record.work_counters.entries[0].min_units = 9;
                }
                _ => unreachable!(),
            }
            assert!(
                receipt
                    .validate_transform_family_coverage_v1(&expected)
                    .is_err(),
                "mutation {mutation}"
            );
        }
        receipt.measurement = Some(original);
        receipt.cpu_calls += 1;
        assert_eq!(
            receipt.validate_transform_family_coverage_v1(&expected),
            Err(CoverageErrorV1::Work)
        );
    }

    #[test]
    fn explicit_transform_context_preserves_inactivity_and_parent_lifetime() {
        let inactive =
            PhaseTimerV1::start_v1(PhaseV1::InitialJoinedTransform).transform_context_v1();
        let observation = ObservationV1::begin_v1();
        inactive
            .enter(KindV1::Base, Ok(FamilyV1::Projection))
            .complete();
        assert_eq!(
            observation
                .finish_v1()
                .measurement_v1()
                .unwrap()
                .phase_tree
                .nodes
                .len(),
            1
        );
        let observation = ObservationV1::begin_v1();
        let timer = PhaseTimerV1::start_v1(PhaseV1::InitialJoinedTransform);
        let old = timer.transform_context_v1();
        timer.complete_v1();
        old.enter(KindV1::Base, Ok(FamilyV1::Projection)).complete();
        let first = observation.finish_v1();
        assert_eq!(
            first.measurement_v1().unwrap().recorder.stale_parent_events,
            1
        );
        let unrelated = ObservationV1::begin_v1();
        old.enter(KindV1::Base, Ok(FamilyV1::Projection)).complete();
        assert_eq!(
            unrelated
                .finish_v1()
                .measurement_v1()
                .unwrap()
                .phase_tree
                .nodes
                .len(),
            1
        );
    }

    #[test]
    fn invalid_attribution_preserves_success_and_is_explicitly_refused() {
        let observation = ObservationV1::begin_v1();
        let timer = PhaseTimerV1::start_v1(PhaseV1::InitialJoinedTransform);
        timer
            .transform_context_v1()
            .enter(KindV1::Base, Err(InvalidAttributionV1))
            .complete();
        timer.complete_v1();
        let receipt = observation.finish_v1();
        let record = receipt.measurement_v1().unwrap();
        assert_eq!(record.outcome, iroha_measurement::RunOutcome::Succeeded);
        let invalid = record
            .phase_tree
            .nodes
            .iter()
            .find(|node| node.label == INVALID_LABEL)
            .unwrap();
        assert_eq!(
            (
                invalid.calls,
                invalid.completed,
                invalid.interrupted,
                invalid.unwound
            ),
            (1, 0, 1, 0)
        );
        assert_eq!(
            receipt.validate_transform_family_coverage_v1(&ExpectedV1::default()),
            Err(CoverageErrorV1::InvalidAttribution)
        );
    }
}
