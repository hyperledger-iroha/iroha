//! Explicit-parent ownership, fixed labels and real recorder lifecycle controls.

use iroha_measurement::{
    CollectingSink, FlowKind, MeasurementRecord, PhaseNode, PhaseParent, RecorderHealth,
    RunContext, RunIdentity, Session, Severity, WorkerDeclaration,
};

use super::commitment_observation::{ContextV1, OperationGuardV1, OperationV1};

const OPERATIONS: [(OperationV1, &str); 7] = [
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
];

fn begin() -> (Session, CollectingSink) {
    let sink = CollectingSink::new();
    let session = Session::begin(
        RunIdentity::new(
            RunContext::unbound(),
            "commitment_context_control",
            FlowKind::Proof,
            "rust.aggregate_commitment_test",
        ),
        "root",
        WorkerDeclaration {
            workers: 1,
            provenance: "test.explicit_owner",
        },
        Box::new(sink.clone()),
    );
    (session, sink)
}

fn finish(session: Session, sink: &CollectingSink) -> MeasurementRecord {
    session.finish();
    let mut records = sink.take();
    assert_eq!(records.len(), 1);
    let record = records.remove(0);
    // These tiny unbound lifecycle records do not claim coverage or identity
    // qualification. Their real accounting must still be well-formed.
    assert!(
        record
            .findings()
            .iter()
            .all(|finding| finding.severity() != Severity::Malformed),
        "{:?}",
        record.findings(),
    );
    record
}

fn node<'a>(record: &'a MeasurementRecord, label: &str) -> (u32, &'a PhaseNode) {
    let mut matches = record
        .phase_tree
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.label == label);
    let (index, node) = matches.next().expect("expected original phase");
    assert!(matches.next().is_none(), "one node per static operation");
    (u32::try_from(index).unwrap(), node)
}

#[test]
fn context_interface_is_opaque_and_operation_guards_stay_on_their_thread() {
    let _: fn(Option<PhaseParent>) -> ContextV1 = ContextV1::from_parent;
    let _: fn(&ContextV1, OperationV1) -> OperationGuardV1 = ContextV1::enter;
    let _: fn(OperationGuardV1) = OperationGuardV1::complete;
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ContextV1>();

    trait AmbiguousIfImplemented<Marker> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfImplemented<u8> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfImplemented<u16> for T {}
    let _ = <OperationGuardV1 as AmbiguousIfImplemented<_>>::check;
}

#[test]
fn seven_static_operations_aggregate_calls_under_the_exact_parent() {
    let (session, sink) = begin();
    let parent = session.enter("base");
    let context = ContextV1::from_parent(Some(parent.parent()));
    for (operation, label) in OPERATIONS {
        assert!(iroha_measurement::text::is_public_label(label));
        for _ in 0..3 {
            context.clone().enter(operation).complete();
        }
    }
    parent.complete();
    let record = finish(session, &sink);
    assert_eq!(record.phase_tree.nodes.len(), 9);
    assert_eq!(record.recorder, RecorderHealth::default());
    let (parent_index, parent) = node(&record, "base");
    let mut children_wall = 0;
    for (_, label) in OPERATIONS {
        let (_, child) = node(&record, label);
        assert_eq!(child.parent, Some(parent_index));
        assert_eq!((child.calls, child.completed, child.interrupted), (3, 3, 0));
        assert_eq!((child.unwound, child.truncated), (0, 0));
        children_wall += child.wall_inclusive_ns;
    }
    // All calls here are sequential; no duration threshold or summed worker
    // wall is substituted for the recorder's actual direct-child accounting.
    assert_eq!(
        parent.wall_exclusive_ns,
        parent.wall_inclusive_ns - children_wall
    );
}

#[test]
fn each_operation_preserves_completed_abandoned_and_unwound_outcomes() {
    let (session, sink) = begin();
    let parent = session.enter("base");
    let context = ContextV1::from_parent(Some(parent.parent()));
    for (operation, _) in OPERATIONS {
        context.enter(operation).complete();
        drop(context.enter(operation));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = context.enter(operation);
            panic!("injected operation-owner unwind");
        }));
        assert!(outcome.is_err());
    }
    parent.complete();
    let record = finish(session, &sink);
    assert_eq!(record.recorder, RecorderHealth::default());
    for (_, label) in OPERATIONS {
        let (_, child) = node(&record, label);
        assert_eq!((child.calls, child.completed, child.interrupted), (3, 1, 2));
        assert_eq!((child.unwound, child.truncated), (1, 0));
    }
}

#[test]
fn inactive_contexts_do_not_adopt_an_observation_on_any_thread() {
    let (session, sink) = begin();
    for context in [ContextV1::default(), ContextV1::from_parent(None)] {
        context.enter(OperationV1::StreamSetup).complete();
        let other = std::thread::spawn(move || {
            let (session, sink) = begin();
            context.enter(OperationV1::RootBinding).complete();
            finish(session, &sink)
        })
        .join()
        .unwrap();
        assert_eq!(other.phase_tree.nodes.len(), 1);
        assert_eq!(other.recorder, RecorderHealth::default());
    }
    let record = finish(session, &sink);
    assert_eq!(record.phase_tree.nodes.len(), 1);
    assert_eq!(record.recorder, RecorderHealth::default());
}

#[test]
fn cloned_context_keeps_its_original_parent_inside_an_unrelated_session() {
    let (session, sink) = begin();
    let parent = session.enter("base");
    let context = ContextV1::from_parent(Some(parent.parent()));
    context.enter(OperationV1::RowAbsorption).complete();
    let other = std::thread::spawn(move || {
        let (session, sink) = begin();
        let unrelated = session.enter("unrelated");
        context.clone().enter(OperationV1::RowAbsorption).complete();
        ContextV1::default()
            .enter(OperationV1::StreamSetup)
            .complete();
        unrelated.complete();
        finish(session, &sink)
    })
    .join()
    .unwrap();
    parent.complete();
    let record = finish(session, &sink);
    let (parent_index, _) = node(&record, "base");
    let (_, child) = node(&record, "RowCommitmentRowAbsorption");
    assert_eq!(child.parent, Some(parent_index));
    assert_eq!((child.calls, child.completed), (2, 2));
    assert_eq!(record.recorder, RecorderHealth::default());
    assert_eq!(other.phase_tree.nodes.len(), 2);
    assert_eq!(other.recorder, RecorderHealth::default());
    assert!(
        other
            .phase_tree
            .nodes
            .iter()
            .all(|node| !node.label.starts_with("RowCommitment"))
    );
}

#[test]
fn ended_parent_retains_original_root_fallback_and_stale_health() {
    let (session, sink) = begin();
    let parent = session.enter("base");
    let context = ContextV1::from_parent(Some(parent.parent()));
    parent.complete();
    let unrelated = session.enter("unrelated");
    context.enter(OperationV1::RootBinding).complete();
    unrelated.complete();
    let record = finish(session, &sink);
    let (_, child) = node(&record, "RowCommitmentRootBinding");
    assert_eq!(child.parent, Some(0));
    assert_eq!((child.calls, child.completed), (1, 1));
    assert_eq!(
        record.recorder,
        RecorderHealth {
            stale_parent_events: 1,
            ..RecorderHealth::default()
        }
    );
}

#[test]
fn ended_session_context_cannot_write_into_a_later_observation() {
    let (first, first_sink) = begin();
    let parent = first.enter("base");
    let context = ContextV1::from_parent(Some(parent.parent()));
    parent.complete();
    let first = finish(first, &first_sink);
    let (second, second_sink) = begin();
    context.enter(OperationV1::RootBinding).complete();
    let other = std::thread::spawn(move || {
        let (session, sink) = begin();
        context.enter(OperationV1::RowAbsorption).complete();
        finish(session, &sink)
    })
    .join()
    .unwrap();
    let second = finish(second, &second_sink);
    assert_eq!(first.phase_tree.nodes.len(), 2);
    for record in [second, other] {
        assert_eq!(record.phase_tree.nodes.len(), 1);
        assert_eq!(record.recorder, RecorderHealth::default());
    }
}
