//! Real-worker controls for explicit diagnostic phase ownership and bounds.

use std::sync::Barrier;

use iroha_measurement::{Finding, PhaseNode, RecorderHealth, Severity};

use crate::privacy_engines::aggregate_stark::commitment_observation::{ContextV1, OperationV1};

use super::*;

fn node<'a>(record: &'a MeasurementRecord, label: &str) -> (u32, &'a PhaseNode) {
    let mut matches = record
        .phase_tree
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.label == label);
    let (index, node) = matches.next().expect("recorded worker phase");
    assert!(matches.next().is_none(), "unique worker phase label");
    (u32::try_from(index).unwrap(), node)
}

fn assert_accounting_valid(record: &MeasurementRecord) {
    // These tiny lifecycle controls intentionally have unbound identities and
    // make no wall-time attribution claim. Their real tree accounting must hold.
    assert!(
        record
            .findings()
            .iter()
            .all(|finding| finding.severity() != Severity::Malformed),
        "{:?}",
        record.findings(),
    );
}

fn commitment_operations() -> [(OperationV1, &'static str); 7] {
    [
        (OperationV1::StreamSetup, "RowCommitmentStreamSetup"),
        (OperationV1::ColumnAdmission, "RowCommitmentColumnAdmission"),
        (OperationV1::RowAbsorption, "RowCommitmentRowAbsorption"),
        (
            OperationV1::FinishPreparation,
            "RowCommitmentFinishPreparation",
        ),
        (
            OperationV1::LeafFinalization,
            "RowCommitmentLeafFinalization",
        ),
        (OperationV1::MerkleReduction, "RowCommitmentMerkleReduction"),
        (OperationV1::RootBinding, "RowCommitmentRootBinding"),
    ]
}

#[test]
fn commitment_context_keeps_exact_base_parent_and_seventy_five_flat_counters() {
    let _: fn(&PhaseTimerV1) -> ContextV1 = PhaseTimerV1::commitment_context_v1;
    let observation = ObservationV1::begin_v1();
    let base = PhaseTimerV1::start_v1(PhaseV1::BaseSampleAndCommit);
    let context = base.commitment_context_v1();
    let inactive = ContextV1::default();

    // This checks the upper bridge, not the commitment operation bodies. The
    // explicit Base parent must survive an unrelated ambient phase. Actual
    // joined commitment controls exercise the lower operation boundaries.
    let aux = PhaseTimerV1::start_v1(PhaseV1::AuxSampleAndCommit);
    for (operation, _) in commitment_operations() {
        context.enter(operation).complete();
    }
    for (operation, _) in commitment_operations() {
        inactive.enter(operation).complete();
    }
    aux.complete_v1();
    let query = PhaseTimerV1::start_v1(PhaseV1::QueryOpenings);
    for (operation, _) in commitment_operations() {
        inactive.enter(operation).complete();
    }
    query.complete_v1();
    base.complete_v1();

    let receipt = observation.finish_v1();
    assert_eq!(receipt.phases.len(), 75);
    assert_eq!(PHASES.len(), 75);
    assert_eq!(PHASE_LABELS_V1.len(), 75);
    for (index, count) in receipt.phases.iter().enumerate() {
        let expected = u64::from(
            [
                PhaseV1::BaseSampleAndCommit as usize,
                PhaseV1::AuxSampleAndCommit as usize,
                PhaseV1::QueryOpenings as usize,
            ]
            .contains(&index),
        );
        assert_eq!(
            (
                count.calls,
                count.completed,
                count.interrupted,
                count.unwound
            ),
            (expected, expected, 0, 0),
        );
    }
    let record = receipt.measurement_v1().unwrap();
    let (base_index, _) = node(record, "BaseSampleAndCommit");
    for (_, label) in commitment_operations() {
        let (_, operation) = node(record, label);
        assert_eq!(operation.parent, Some(base_index));
        assert_eq!(
            (
                operation.calls,
                operation.completed,
                operation.interrupted,
                operation.unwound,
            ),
            (1, 1, 0, 0),
        );
        assert!(!PHASE_LABELS_V1.contains(&label));
    }
    assert_eq!(record.recorder, RecorderHealth::default());
    assert_accounting_valid(record);
}

#[test]
fn inactive_timer_commitment_context_cannot_adopt_a_later_observation() {
    let inactive = PhaseTimerV1::start_v1(PhaseV1::BaseSampleAndCommit);
    let context = inactive.commitment_context_v1();
    let observation = ObservationV1::begin_v1();
    let active = PhaseTimerV1::start_v1(PhaseV1::BaseSampleAndCommit);
    for (operation, _) in commitment_operations() {
        context.enter(operation).complete();
    }
    active.complete_v1();
    inactive.complete_v1();
    let receipt = observation.finish_v1();
    assert_eq!(
        receipt.phases[PhaseV1::BaseSampleAndCommit as usize].calls,
        1
    );
    assert_eq!(
        receipt.phases.iter().map(|phase| phase.calls).sum::<u64>(),
        1
    );
    let record = receipt.measurement_v1().unwrap();
    for (_, label) in commitment_operations() {
        assert!(
            record
                .phase_tree
                .nodes
                .iter()
                .all(|node| node.label != label)
        );
    }
    assert_eq!(record.recorder, RecorderHealth::default());
    assert_accounting_valid(record);
}

#[test]
fn worker_interface_is_opaque_and_only_parent_tokens_cross_threads() {
    let _: fn(&PhaseTimerV1) -> WorkerPhaseParentV1 = PhaseTimerV1::worker_parent_v1;
    let _: fn(&WorkerPhaseParentV1, PhaseV1) -> WorkerPhaseV1 = WorkerPhaseParentV1::enter_v1;
    let _: fn(&WorkerPhaseV1) -> WorkerPhaseParentV1 = WorkerPhaseV1::worker_parent_v1;
    let _: fn(WorkerPhaseV1) = WorkerPhaseV1::complete_v1;
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WorkerPhaseParentV1>();

    // Match the shared recorder's compile-time negative assertion: a Send or
    // Sync guard makes the marker parameter ambiguous and fails compilation.
    trait AmbiguousIfImplemented<Marker> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfImplemented<u8> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfImplemented<u16> for T {}
    let _ = <WorkerPhaseV1 as AmbiguousIfImplemented<_>>::check;
}

#[test]
fn rayon_workers_keep_explicit_nested_parents_and_completion() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let observation = ObservationV1::begin_v1();
    let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = outer.worker_parent_v1();
    let barrier = Barrier::new(4);
    pool.scope(|scope| {
        for _ in 0..4 {
            let parent = parent.clone();
            let barrier = &barrier;
            scope.spawn(move |_| {
                assert!(rayon::current_thread_index().is_some());
                let worker = parent.enter_v1(PhaseV1::CompositionBaseReplay);
                let nested = worker
                    .worker_parent_v1()
                    .enter_v1(PhaseV1::CompositionInverseTransform);
                // Every actual worker holds both guards at this event. There
                // is no timing threshold or assumed task scheduling order.
                barrier.wait();
                nested.complete_v1();
                worker.complete_v1();
            });
        }
    });
    outer.complete_v1();
    let receipt = observation.finish_v1();
    let record = receipt.measurement_v1().unwrap();
    let (outer_index, outer) = node(record, "Composition");
    let (worker_index, worker) = node(record, "CompositionBaseReplay");
    let (_, nested) = node(record, "CompositionInverseTransform");
    assert_eq!(worker.parent, Some(outer_index));
    assert_eq!(nested.parent, Some(worker_index));
    for phase in [worker, nested] {
        assert_eq!(
            (
                phase.calls,
                phase.completed,
                phase.interrupted,
                phase.unwound
            ),
            (4, 4, 0, 0),
        );
        assert!(phase.active_threads_max >= 4);
    }
    assert_eq!(outer.peak_concurrent_children, 4);
    let covered = outer.wall_inclusive_ns - outer.wall_exclusive_ns;
    assert!(covered <= worker.wall_inclusive_ns);
    assert!(worker.wall_inclusive_ns <= covered.saturating_mul(4));
    assert_eq!(record.recorder, RecorderHealth::default());
    assert_eq!(receipt.phases[PhaseV1::Composition as usize].calls, 1);
    assert_eq!(
        receipt.phases[PhaseV1::CompositionBaseReplay as usize].calls,
        0
    );
    assert_eq!(
        receipt.phases[PhaseV1::CompositionInverseTransform as usize].calls,
        0
    );
    assert_accounting_valid(record);
}

#[test]
fn worker_error_and_unwind_keep_original_phase_outcomes() {
    fn error(parent: &WorkerPhaseParentV1) -> Result<(), ()> {
        let _guard = parent.enter_v1(PhaseV1::CompositionBaseReplay);
        Err(())
    }

    let observation = ObservationV1::begin_v1();
    let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = outer.worker_parent_v1();
    std::thread::spawn(move || {
        parent
            .enter_v1(PhaseV1::CompositionBaseReplay)
            .complete_v1();
        assert!(error(&parent).is_err());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = parent.enter_v1(PhaseV1::CompositionBaseReplay);
                panic!("synthetic public worker unwind");
            }))
            .is_err(),
        );
    })
    .join()
    .unwrap();
    outer.complete_v1();
    // As with serial phases, the caller owns propagation of a caught failure;
    // the worker guard records counters and accepts no formatted error payload.
    observation.record_failure_v1("worker", "error");
    let receipt = observation.finish_v1();
    let record = receipt.measurement_v1().unwrap();
    let (_, worker) = node(record, "CompositionBaseReplay");
    assert_eq!(
        (
            worker.calls,
            worker.completed,
            worker.interrupted,
            worker.unwound
        ),
        (3, 1, 2, 1),
    );
    assert_eq!(record.outcome, iroha_measurement::RunOutcome::Failed);
    assert_eq!(record.recorder, RecorderHealth::default());
    assert_accounting_valid(record);
}

#[test]
fn inactive_parent_does_not_adopt_an_unrelated_observation() {
    let inactive = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = inactive.worker_parent_v1();
    inactive.complete_v1();
    let observation = ObservationV1::begin_v1();
    let unrelated = std::thread::spawn(move || {
        let observation = ObservationV1::begin_v1();
        let worker = parent.enter_v1(PhaseV1::CompositionBaseReplay);
        worker
            .worker_parent_v1()
            .enter_v1(PhaseV1::CompositionInverseTransform)
            .complete_v1();
        worker.complete_v1();
        observation.finish_v1().measurement_v1().unwrap().clone()
    })
    .join()
    .unwrap();
    let receipt = observation.finish_v1();
    for record in [receipt.measurement_v1().unwrap(), &unrelated] {
        assert_eq!(record.phase_tree.nodes.len(), 1);
        assert_eq!(record.recorder, RecorderHealth::default());
        assert_accounting_valid(record);
    }
}

#[test]
fn ended_parent_is_counted_and_attaches_at_the_original_root() {
    let observation = ObservationV1::begin_v1();
    let ended = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = ended.worker_parent_v1();
    ended.complete_v1();
    let unrelated = PhaseTimerV1::start_v1(PhaseV1::QueryOpenings);
    std::thread::spawn(move || {
        parent
            .enter_v1(PhaseV1::CompositionBaseReplay)
            .complete_v1();
    })
    .join()
    .unwrap();
    unrelated.complete_v1();
    let receipt = observation.finish_v1();
    let record = receipt.measurement_v1().unwrap();
    let (_, worker) = node(record, "CompositionBaseReplay");
    assert_eq!(worker.parent, Some(0));
    assert_eq!(worker.completed, 1);
    assert_eq!(
        record.recorder,
        RecorderHealth {
            stale_parent_events: 1,
            ..RecorderHealth::default()
        },
    );
    // Existing recorder semantics count this fallback without inventing a new
    // rejection rule. Real callers must join workers before closing the parent.
    assert_accounting_valid(record);
}

#[test]
fn ended_observation_token_cannot_write_into_later_observations() {
    let first = ObservationV1::begin_v1();
    let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = outer.worker_parent_v1();
    outer.complete_v1();
    let first = first.finish_v1();
    let second = ObservationV1::begin_v1();
    let unrelated = std::thread::spawn(move || {
        let observation = ObservationV1::begin_v1();
        parent
            .enter_v1(PhaseV1::CompositionBaseReplay)
            .complete_v1();
        observation.finish_v1().measurement_v1().unwrap().clone()
    })
    .join()
    .unwrap();
    let second = second.finish_v1();
    assert_eq!(first.measurement_v1().unwrap().phase_tree.nodes.len(), 2);
    for record in [second.measurement_v1().unwrap(), &unrelated] {
        assert_eq!(record.phase_tree.nodes.len(), 1);
        assert_eq!(record.recorder, RecorderHealth::default());
        assert_accounting_valid(record);
    }
}

#[test]
fn worker_open_span_bound_is_retained_without_extra_labels() {
    let observation = ObservationV1::begin_v1();
    let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
    let parent = outer.worker_parent_v1();
    std::thread::spawn(move || {
        let mut guards = Vec::new();
        for _ in 0..=iroha_measurement::recorder::MAX_OPEN_SPANS {
            guards.push(parent.enter_v1(PhaseV1::CompositionBaseReplay));
        }
        while let Some(guard) = guards.pop() {
            guard.complete_v1();
        }
    })
    .join()
    .unwrap();
    outer.complete_v1();
    let receipt = observation.finish_v1();
    let record = receipt.measurement_v1().unwrap();
    assert_eq!(record.phase_tree.nodes.len(), 3);
    assert_eq!(
        record.recorder,
        RecorderHealth {
            // The root and the initiating timer already own two slots.
            span_overflow_events: 3,
            ..RecorderHealth::default()
        },
    );
    assert!(record.findings().contains(&Finding::RecorderOverflow));
    assert_accounting_valid(record);
}

#[test]
fn worker_node_bound_uses_only_the_closed_phase_inventory() {
    let observation = ObservationV1::begin_v1();
    let outer = PhaseTimerV1::start_v1(PhaseV1::Preparation);
    let parent = outer.worker_parent_v1();
    std::thread::spawn(move || {
        // Distinct parent/label pairs fill the real table while at most four
        // guards are open. The labels remain the original fixed 75 variants.
        for phase in PHASES {
            let branch = parent.enter_v1(phase);
            let branch_parent = branch.worker_parent_v1();
            for child in PHASES {
                branch_parent.enter_v1(child).complete_v1();
            }
            branch.complete_v1();
        }
    })
    .join()
    .unwrap();
    outer.complete_v1();
    let receipt = observation.finish_v1();
    let record = receipt.measurement_v1().unwrap();
    assert_eq!(
        record.phase_tree.nodes.len(),
        iroha_measurement::recorder::MAX_NODES
    );
    assert!(record.recorder.node_overflow_events > 0);
    assert_eq!(record.recorder.span_overflow_events, 0);
    assert_eq!(record.recorder.stale_parent_events, 0);
    assert_eq!(record.recorder.forced_closes, 0);
    assert!(
        record.phase_tree.nodes[1..]
            .iter()
            .all(|node| PHASE_LABELS_V1.contains(&node.label.as_str())),
    );
    assert!(record.findings().contains(&Finding::RecorderOverflow));
    assert_accounting_valid(record);
}
