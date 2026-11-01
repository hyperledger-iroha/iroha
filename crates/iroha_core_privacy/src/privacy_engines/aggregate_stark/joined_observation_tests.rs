//! Real joined commitments, original failures and clearing under explicit phases.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use iroha_measurement::{
    CollectingSink, FlowKind, MeasurementRecord, PhaseNode, RecorderHealth, RunContext,
    RunIdentity, Session, Severity, WorkerDeclaration,
};

use super::*;
use crate::privacy_engines::aggregate_stark::{
    commitment_observation::ContextV1,
    field_column_erasure,
    joined_trace::{
        JoinedTraceBatchCoordinatesV1, JoinedTraceColumnKindV1, JoinedTraceCommitmentPlanV1,
    },
};
use crate::privacy_engines::transparent_stark::{GOLDILOCKS_GENERATOR_V1, GOLDILOCKS_MODULUS_V1};

const ROWS: usize = 1 << 13;
const WIDTH: usize = 9;
const NATIVE_LOG: u8 = 10;
const COMMON_LOG: u8 = 13;
const COEFFICIENTS: usize = (1 << NATIVE_LOG) + 4;

fn begin_observation(workers: u32) -> (Session, CollectingSink) {
    let sink = CollectingSink::new();
    let session = Session::begin(
        RunIdentity::new(
            RunContext::unbound(),
            "joined_commitment_control",
            FlowKind::Proof,
            "rust.aggregate_commitment_test",
        ),
        "root",
        WorkerDeclaration {
            workers,
            provenance: "test.actual_rayon_pool",
        },
        Box::new(sink.clone()),
    );
    (session, sink)
}

fn finish_observation(session: Session, sink: &CollectingSink) -> MeasurementRecord {
    session.finish();
    let mut records = sink.take();
    assert_eq!(records.len(), 1);
    let record = records.remove(0);
    assert!(
        record
            .findings()
            .iter()
            .all(|finding| finding.severity() != Severity::Malformed),
        "{:?}",
        record.findings(),
    );
    assert_eq!(record.recorder, RecorderHealth::default());
    record
}

fn phase<'a>(record: &'a MeasurementRecord, label: &str) -> &'a PhaseNode {
    let mut matches = record
        .phase_tree
        .nodes
        .iter()
        .filter(|node| node.label == label);
    let node = matches.next().expect("actual operation recorded");
    assert!(
        matches.next().is_none(),
        "static operation aggregates calls"
    );
    let parent = record
        .phase_tree
        .nodes
        .iter()
        .position(|node| node.label == "base")
        .unwrap();
    assert_eq!(node.parent, Some(u32::try_from(parent).unwrap()));
    assert_eq!(node.truncated, 0);
    node
}

fn assert_absent(record: &MeasurementRecord, labels: &[&str]) {
    for label in labels {
        assert!(
            record
                .phase_tree
                .nodes
                .iter()
                .all(|node| node.label != *label),
            "{label}"
        );
    }
}

fn fixture() -> (AggregateStarkDomainsV1, JoinedTraceCommitmentPlanV1) {
    let parameters = AggregateStarkParametersV1 {
        proof_magic: *b"OBS1",
        proof_version: 1,
        fri_commitment_layout: AggregateFriCommitmentLayoutV1::Paired,
        security_lanes: 2,
        query_count: FASTPQ_QUERY_COUNT_V1 as usize,
        blowup_log2: 3,
        terminal_log2: 3,
        terminal_degree_bound: 3,
        composition_degree_chunks: 3,
        minimum_trace_log2: 5,
        maximum_trace_log2: NATIVE_LOG,
        maximum_trace_groups: 4,
        maximum_segment_instances: 4,
        maximum_base_columns_per_instance: 16,
        maximum_aux_columns_per_instance: 16,
        maximum_proof_bytes: 1 << 20,
    };
    let domains = AggregateStarkDomainsV1 {
        digest_context: context(),
        base_leaf: b"observed-joined-base-leaf",
        base_node: b"observed-joined-base-node",
        aux_leaf: b"observed-joined-aux-leaf",
        aux_node: b"observed-joined-aux-node",
        composition_leaf: b"observed-joined-composition-leaf",
        composition_node: b"observed-joined-composition-node",
        fri_leaf: b"observed-joined-fri-leaf",
        fri_node: b"observed-joined-fri-node",
        layout_label: b"observed-joined-layout",
        base_root_label: b"observed-joined-base-root",
        aux_root_label: b"observed-joined-aux-root",
        composition_root_label: b"observed-joined-composition-root",
        fri_root_label: b"observed-joined-fri-root",
        fri_beta_label: b"observed-joined-fri-beta",
        query_seed: b"observed-joined-query",
    };
    let layout = AggregateProofLayoutV1::new(
        parameters,
        vec![AggregateTraceGroupLayoutV1 {
            native_trace_log2: NATIVE_LOG,
            segment_instances: 1,
            base_width: WIDTH,
            aux_width: 1,
        }],
    )
    .unwrap();
    let plan =
        JoinedTraceCommitmentPlanV1::new_v1(parameters, &layout, JoinedTraceColumnKindV1::Base)
            .unwrap();
    (domains, plan)
}

fn source(column: usize) -> Vec<F> {
    let mut coefficients = vec![F::ZERO; COEFFICIENTS];
    coefficients[0] = F((column + 3) as u64);
    coefficients[1] = F(5);
    coefficients[COEFFICIENTS - 1] = F((column + 11) as u64);
    coefficients
}

fn evaluate(
    columns: &[ZeroizingFieldColumnV1],
    native: u8,
    common: u8,
    selected: Option<&[usize]>,
    coordinates: JoinedTraceBatchCoordinatesV1,
) -> Result<Vec<ZeroizingFieldColumnV1>, AggregateStarkErrorV1> {
    assert_eq!((native, common), (NATIVE_LOG, COMMON_LOG));
    assert_eq!(coordinates.group, 0);
    assert!(coordinates.start < WIDTH && coordinates.start.is_multiple_of(8));
    assert_eq!(coordinates.end, (coordinates.start + 8).min(WIDTH));
    assert_eq!(columns.len(), coordinates.end - coordinates.start);
    columns
        .iter()
        .enumerate()
        .map(|(offset, column)| {
            // Public coordinates must describe the original ordered allocation,
            // including its populated high masking tail, not a relabelled batch.
            let original = coordinates.start + offset;
            assert_eq!(column.len(), COEFFICIENTS);
            assert_eq!(column[0], F((original + 3) as u64));
            assert_eq!(column[COEFFICIENTS - 1], F((original + 11) as u64));
            let full = ZeroizingFieldColumnV1(
                masked_trace_coefficients_on_coset_v1(column, native, common)
                    .map_err(map_transparent_error_v1)?,
            );
            Ok(match selected {
                Some(indices) => {
                    ZeroizingFieldColumnV1(indices.iter().map(|&row| full[row]).collect())
                }
                None => full,
            })
        })
        .collect()
}

fn independent_tree(
    domains: &AggregateStarkDomainsV1,
    queries: &[usize],
) -> (PrivacyOuterMerkleTreeV1, BTreeMap<usize, Vec<F>>) {
    let root = goldilocks_primitive_root_v1(COMMON_LOG).unwrap();
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    let mut opened = BTreeMap::new();
    let leaves = (0..ROWS)
        .map(|index| {
            // Evaluate the three explicitly populated monomials independently of
            // every FFT, row-stream and retained-cut implementation under test.
            let high = x.pow((COEFFICIENTS - 1) as u128);
            let row = (0..WIDTH)
                .map(|column| {
                    F((column + 3) as u64)
                        .add(F(5).mul(x))
                        .add(F((column + 11) as u64).mul(high))
                })
                .collect::<Vec<_>>();
            let leaf = row_leaf_hash_v1(
                domains.digest_context,
                domains.base_leaf,
                JOINED_TRACE_GROUP_MARKER_V1,
                index,
                &row,
            )
            .unwrap();
            if queries.contains(&index) {
                opened.insert(index, row);
            }
            x = x.mul(root);
            leaf
        })
        .collect();
    (
        PrivacyOuterMerkleTreeV1::from_leaves(leaves, domains.digest_context, domains.base_node)
            .unwrap(),
        opened,
    )
}

#[test]
fn actual_joined_two_tile_commitment_preserves_roots_cuts_and_seven_node_census() {
    let (domains, plan) = fixture();
    let queries = [0, 4095, 4096, ROWS - 1];
    let (expected, opened) = independent_tree(&domains, &queries);
    let frontier = canonical_multiproof_frontier_v1(&expected, ROWS, &queries).unwrap();
    for workers in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let (session, sink) = begin_observation(workers as u32);
                let parent = session.enter("base");
                let observation = ContextV1::from_parent(Some(parent.parent()));
                let mut source_order = Vec::new();
                let mut batch_widths = Vec::new();
                let mut initial_coordinates = Vec::new();
                let census = Census::begin();
                let (actual, cut) = plan
                    .commit_retained_replayed_v1(
                        &domains,
                        &[],
                        None,
                        |group, column| {
                            assert_eq!(group, 0);
                            source_order.push(column);
                            Ok(source(column))
                        },
                        |columns, native, common, selected, coordinates| {
                            assert!(selected.is_none());
                            batch_widths.push(columns.len());
                            initial_coordinates.push(coordinates);
                            evaluate(columns, native, common, selected, coordinates)
                        },
                        observation,
                    )
                    .unwrap();
                assert_eq!(census.finish(), vec![4096]);
                assert_eq!(source_order, (0..WIDTH).collect::<Vec<_>>());
                assert_eq!(batch_widths, [8, 1]);
                assert_eq!(
                    initial_coordinates,
                    [
                        JoinedTraceBatchCoordinatesV1 {
                            group: 0,
                            start: 0,
                            end: 8
                        },
                        JoinedTraceBatchCoordinatesV1 {
                            group: 0,
                            start: 8,
                            end: 9
                        },
                    ]
                );
                assert_eq!(actual.commitment.root, expected.root());
                assert!(actual.commitment.frontier.is_empty());
                assert!(actual.opened_rows.is_empty());
                let cut = cut.unwrap();
                cut.check_root_v1(ROWS, expected.root()).unwrap();

                // Both unobserved actual calls run while the original session is
                // still live. Neither may adopt it or inflate its operation calls.
                let (inactive, inactive_cut) = plan
                    .commit_retained_replayed_v1(
                        &domains,
                        &[],
                        None,
                        |_, column| Ok(source(column)),
                        evaluate,
                        ContextV1::default(),
                    )
                    .unwrap();
                assert_eq!(inactive, actual);
                inactive_cut
                    .unwrap()
                    .check_root_v1(ROWS, expected.root())
                    .unwrap();
                let mut replay_coordinates = Vec::new();
                let mut selected_widths = Vec::new();
                let (replay_result, erased) = field_column_erasure::observe_v1(|| {
                    plan.commit_retained_replayed_v1(
                        &domains,
                        &queries,
                        Some(&cut),
                        |_, column| Ok(source(column)),
                        |columns, native, common, selected, coordinates| {
                            replay_coordinates.push(coordinates);
                            selected_widths.push(selected.expect("actual retained replay").len());
                            evaluate(columns, native, common, selected, coordinates)
                        },
                        ContextV1::default(),
                    )
                });
                let (replayed, absent_cut) = replay_result.unwrap();
                assert_eq!(replay_coordinates, initial_coordinates);
                assert_eq!(selected_widths.len(), 2);
                assert_eq!(selected_widths[0], selected_widths[1]);
                // Each original coefficient, full evaluator output and selected
                // output keeps its existing clearing owner after the callback.
                assert_eq!(erased.len(), 3 * WIDTH);
                assert!(
                    erased
                        .iter()
                        .all(|entry| { entry.nonzero_before > 0 && entry.nonzero_after == 0 })
                );
                assert_eq!(
                    erased.iter().map(|entry| entry.cells).sum::<usize>(),
                    WIDTH * (COEFFICIENTS + ROWS + selected_widths[0])
                );
                assert!(absent_cut.is_none());
                assert_eq!(replayed.commitment.root, expected.root());
                assert_eq!(replayed.commitment.frontier, frontier);
                assert_eq!(replayed.opened_rows, opened);
                parent.complete();
                let record = finish_observation(session, &sink);
                assert_eq!(record.phase_tree.nodes.len(), 9);
                let mut children_wall = 0;
                let mut calls = 0;
                for (label, expected_calls) in [
                    ("RowCommitmentStreamSetup", 1),
                    ("RowCommitmentColumnAdmission", 2),
                    ("RowCommitmentRowAbsorption", 2),
                    ("RowCommitmentFinishPreparation", 1),
                    ("RowCommitmentLeafFinalization", 2),
                    ("RowCommitmentMerkleReduction", 2),
                    ("RowCommitmentRootBinding", 1),
                ] {
                    let node = phase(&record, label);
                    assert_eq!(
                        (node.calls, node.completed, node.interrupted, node.unwound),
                        (expected_calls, expected_calls, 0, 0)
                    );
                    children_wall += node.wall_inclusive_ns;
                    calls += node.calls;
                }
                assert_eq!(calls, 11);
                let parent = record
                    .phase_tree
                    .nodes
                    .iter()
                    .find(|node| node.label == "base")
                    .unwrap();
                assert_eq!(
                    parent.wall_exclusive_ns,
                    parent.wall_inclusive_ns - children_wall
                );
            });
    }
}

fn row_owner(rows: usize, observation: ContextV1) -> StreamingRowCommitmentV1 {
    let owner = StreamingRowCommitmentV1::new(context(), b"leaf", NODE, 0, rows, 2, &[]).unwrap();
    let pointer = owner.digest_streams.as_ptr();
    let capacity = owner.digest_streams.capacity();
    let owner = owner.with_observation_v1(observation);
    assert_eq!(owner.digest_streams.as_ptr(), pointer);
    assert_eq!(owner.digest_streams.capacity(), capacity);
    owner
}

#[test]
fn late_field_refusal_precedes_absorption_and_valid_retry_keeps_original_root() {
    let columns = [vec![F(3); 8], vec![F(7); 8]];
    let mut expected = row_owner(8, ContextV1::default());
    expected.absorb_columns_v1(&columns).unwrap();
    let expected = expected.finish_retaining_cut_v1().unwrap().0;
    let (session, sink) = begin_observation(1);
    let parent = session.enter("base");
    let mut owner = row_owner(8, ContextV1::from_parent(Some(parent.parent())));
    let mut malformed = columns.clone();
    malformed[1][7] = F(GOLDILOCKS_MODULUS_V1);
    assert_eq!(
        owner.absorb_columns_v1(&malformed),
        Err(AggregateStarkErrorV1::NonCanonicalField)
    );
    assert_eq!(owner.received_columns, 0);
    assert!(!owner.failed);
    owner.absorb_columns_v1(&columns).unwrap();
    let census = Census::begin();
    let (actual, cut) = owner.finish_retaining_cut_v1().unwrap();
    assert_eq!(census.finish(), vec![8]);
    assert_eq!(actual, expected);
    cut.check_root_v1(8, actual.commitment.root).unwrap();
    parent.complete();
    let record = finish_observation(session, &sink);
    let admission = phase(&record, "RowCommitmentColumnAdmission");
    assert_eq!(
        (admission.calls, admission.completed, admission.interrupted),
        (2, 1, 1)
    );
    let absorption = phase(&record, "RowCommitmentRowAbsorption");
    assert_eq!((absorption.calls, absorption.completed), (1, 1));
    assert_absent(&record, &["RowCommitmentStreamSetup"]);
}

#[test]
fn partial_hash_failure_preserves_poison_and_original_field_owner_clearing() {
    for workers in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let mut baseline = row_owner(2048, ContextV1::default());
                baseline.digest_streams[1025].update(&[0; 8]).unwrap();
                let original = baseline
                    .absorb_columns_v1(&[vec![F(3); 2048], vec![F(5); 2048]])
                    .err()
                    .unwrap();
                let (session, sink) = begin_observation(workers as u32);
                let parent = session.enter("base");
                let mut owner = row_owner(2048, ContextV1::from_parent(Some(parent.parent())));
                owner.digest_streams[1025].update(&[0; 8]).unwrap();
                let (error, erased) = field_column_erasure::observe_v1(|| {
                    let columns = [
                        ZeroizingFieldColumnV1(vec![F(3); 2048]),
                        ZeroizingFieldColumnV1(vec![F(5); 2048]),
                    ];
                    owner.absorb_columns_v1(&columns).err().unwrap()
                });
                assert_eq!(error, original);
                assert!(owner.failed);
                assert_eq!(owner.received_columns, 0);
                assert!(owner.opened_rows.is_empty());
                assert_eq!(erased.len(), 2);
                assert!(erased.iter().all(|entry| entry.cells == 2048
                    && entry.nonzero_before == 2048
                    && entry.nonzero_after == 0));
                assert!(matches!(
                    owner.finish_retaining_cut_v1(),
                    Err(AggregateStarkErrorV1::InvalidLayout)
                ));
                parent.complete();
                let record = finish_observation(session, &sink);
                assert_eq!(phase(&record, "RowCommitmentColumnAdmission").completed, 1);
                for label in [
                    "RowCommitmentRowAbsorption",
                    "RowCommitmentFinishPreparation",
                ] {
                    let node = phase(&record, label);
                    assert_eq!(
                        (node.calls, node.completed, node.interrupted, node.unwound),
                        (1, 0, 1, 0)
                    );
                }
                assert_absent(
                    &record,
                    &[
                        "RowCommitmentLeafFinalization",
                        "RowCommitmentMerkleReduction",
                        "RowCommitmentRootBinding",
                    ],
                );
            });
    }
}

fn populated_streams(rows: usize) -> Vec<PrivacyOuterLastFieldStreamV1> {
    let catalog = TransparentStarkDigestContextV1::catalog_v1();
    let domain = context()
        .domain_v1(&catalog, b"leaf", b"rows", 0, 0, 0)
        .unwrap();
    let prefix = PrivacyOuterDomainPrefixV1::new(domain).unwrap();
    (0..rows)
        .map(|index| {
            let mut stream = prefix
                .last_field_stream_at_with_counter(index as u64, 0, &[], 8)
                .unwrap();
            stream.update(&(index as u64 + 1).to_le_bytes()).unwrap();
            stream
        })
        .collect()
}

fn finalize(
    stream: &mut PrivacyOuterLastFieldStreamV1,
) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1> {
    stream
        .finalize_in_place_v1()
        .map_err(map_digest_stream_error_v1)
        .map_err(map_transparent_error_v1)
}

#[test]
fn actual_worker_error_and_panic_leave_failed_leaf_phase_and_clear_original_tile() {
    for workers in [1, 4] {
        for panic_at_row in [false, true] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let (session, sink) = begin_observation(workers as u32);
                    let parent = session.enter("base");
                    let observation = ContextV1::from_parent(Some(parent.parent()));
                    let mut streams = populated_streams(32);
                    let mut acc = accumulator(32, &[0, 31]);
                    let census = Census::begin();
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        finish_rows_with_v1(
                            &mut acc,
                            &mut streams,
                            |row, stream| {
                                if row == 3 {
                                    assert!(
                                        !panic_at_row,
                                        "injected original worker finalization unwind"
                                    );
                                    return Err(AggregateStarkErrorV1::NonCanonicalField);
                                }
                                if row == 7 {
                                    return Err(AggregateStarkErrorV1::InternalInvariant);
                                }
                                finalize(stream)
                            },
                            &observation,
                        )
                    }));
                    if panic_at_row {
                        assert!(outcome.is_err());
                    } else {
                        assert_eq!(
                            outcome.unwrap(),
                            Err(AggregateStarkErrorV1::NonCanonicalField)
                        );
                    }
                    assert_eq!(acc.next_leaf, 0);
                    assert!(acc.finish().is_err());
                    assert_eq!(census.finish(), vec![32]);
                    parent.complete();
                    let record = finish_observation(session, &sink);
                    let leaf = phase(&record, "RowCommitmentLeafFinalization");
                    assert_eq!(
                        (leaf.calls, leaf.completed, leaf.interrupted, leaf.unwound),
                        (1, 0, 1, u64::from(panic_at_row))
                    );
                    assert_absent(
                        &record,
                        &["RowCommitmentMerkleReduction", "RowCommitmentRootBinding"],
                    );
                });
        }
    }
}

#[test]
fn coordinator_operation_cannot_finish_before_the_real_worker_is_released() {
    for workers in [1, 4] {
        let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let release_receiver = Mutex::new(release_receiver);
        let returned = Arc::new(AtomicBool::new(false));
        let child_returned = Arc::clone(&returned);
        let thread = std::thread::spawn(move || {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let (session, sink) = begin_observation(workers as u32);
                    let parent = session.enter("base");
                    let observation = ContextV1::from_parent(Some(parent.parent()));
                    let mut streams = populated_streams(32);
                    let mut acc = accumulator(32, &[]);
                    let census = Census::begin();
                    finish_rows_with_v1(
                        &mut acc,
                        &mut streams,
                        |row, stream| {
                            if row == 3 {
                                entered_sender.send(()).unwrap();
                                release_receiver
                                    .lock()
                                    .unwrap()
                                    .recv_timeout(std::time::Duration::from_secs(20))
                                    .expect("test releases real worker");
                            }
                            finalize(stream)
                        },
                        &observation,
                    )
                    .unwrap();
                    child_returned.store(true, Ordering::Release);
                    assert_eq!(acc.next_leaf, 32);
                    assert!(acc.finish().is_ok());
                    assert_eq!(census.finish(), vec![32]);
                    parent.complete();
                    finish_observation(session, &sink)
                })
        });
        let entered = entered_receiver.recv_timeout(std::time::Duration::from_secs(20));
        let returned_before_release = returned.load(Ordering::Acquire);
        // Release before asserting, so a failed fixture assertion cannot leave
        // its worker deliberately blocked. The timeout is a fixture bound only.
        let released = release_sender.send(());
        let record = thread.join().unwrap();
        entered.expect("real finalize callback reached its synchronization event");
        released.unwrap();
        assert!(!returned_before_release);
        assert!(returned.load(Ordering::Acquire));
        for label in [
            "RowCommitmentLeafFinalization",
            "RowCommitmentMerkleReduction",
        ] {
            let node = phase(&record, label);
            assert_eq!((node.calls, node.completed, node.interrupted), (1, 1, 0));
        }
    }
}

#[test]
fn failed_merkle_reduction_does_not_complete_or_publish_a_root_and_clears_tile() {
    let (session, sink) = begin_observation(1);
    let parent = session.enter("base");
    let observation = ContextV1::from_parent(Some(parent.parent()));
    let mut streams = populated_streams(32);
    let mut acc = accumulator(32, &[]);
    acc.node_prefixes.clear();
    let census = Census::begin();
    assert_eq!(
        finish_rows_with_v1(
            &mut acc,
            &mut streams,
            |_, stream| finalize(stream),
            &observation
        ),
        Err(AggregateStarkErrorV1::InternalInvariant)
    );
    assert_eq!(acc.next_leaf, 0);
    assert!(acc.finish().is_err());
    assert_eq!(census.finish(), vec![32]);
    parent.complete();
    let record = finish_observation(session, &sink);
    assert_eq!(phase(&record, "RowCommitmentLeafFinalization").completed, 1);
    let merkle = phase(&record, "RowCommitmentMerkleReduction");
    assert_eq!(
        (merkle.calls, merkle.completed, merkle.interrupted),
        (1, 0, 1)
    );
    assert_absent(&record, &["RowCommitmentRootBinding"]);
}

#[test]
fn original_source_and_evaluator_errors_leave_no_commitment_and_clear_columns() {
    let (domains, plan) = fixture();
    for fail_source in [false, true] {
        let (session, sink) = begin_observation(1);
        let parent = session.enter("base");
        let observation = ContextV1::from_parent(Some(parent.parent()));
        let mut source_order = Vec::new();
        let mut evaluated_coordinates = Vec::new();
        let (result, erased) = field_column_erasure::observe_v1(|| {
            plan.commit_retained_replayed_v1(
                &domains,
                &[],
                None,
                |group, column| {
                    assert_eq!(group, 0);
                    source_order.push(column);
                    if fail_source && column == 8 {
                        return Err(AggregateStarkErrorV1::AllocationFailure);
                    }
                    Ok(source(column))
                },
                |columns, native, common, selected, coordinates| {
                    evaluated_coordinates.push(coordinates);
                    if !fail_source {
                        return Err(AggregateStarkErrorV1::AllocationFailure);
                    }
                    evaluate(columns, native, common, selected, coordinates)
                },
                observation,
            )
        });
        assert!(matches!(
            result,
            Err(AggregateStarkErrorV1::AllocationFailure)
        ));
        assert_eq!(
            source_order,
            (0..if fail_source { 9 } else { 8 }).collect::<Vec<_>>()
        );
        assert_eq!(
            evaluated_coordinates,
            [JoinedTraceBatchCoordinatesV1 {
                group: 0,
                start: 0,
                end: 8
            }]
        );
        assert_eq!(erased.len(), if fail_source { 16 } else { 8 });
        assert!(
            erased
                .iter()
                .all(|entry| entry.nonzero_before > 0 && entry.nonzero_after == 0)
        );
        assert_eq!(
            erased
                .iter()
                .filter(|entry| entry.cells == COEFFICIENTS)
                .count(),
            8
        );
        assert_eq!(
            erased.iter().filter(|entry| entry.cells == ROWS).count(),
            if fail_source { 8 } else { 0 }
        );
        parent.complete();
        let record = finish_observation(session, &sink);
        assert_eq!(phase(&record, "RowCommitmentStreamSetup").completed, 1);
        if fail_source {
            assert_eq!(phase(&record, "RowCommitmentColumnAdmission").completed, 1);
            assert_eq!(phase(&record, "RowCommitmentRowAbsorption").completed, 1);
        } else {
            assert_absent(
                &record,
                &["RowCommitmentColumnAdmission", "RowCommitmentRowAbsorption"],
            );
        }
        assert_absent(
            &record,
            &[
                "RowCommitmentFinishPreparation",
                "RowCommitmentLeafFinalization",
                "RowCommitmentMerkleReduction",
                "RowCommitmentRootBinding",
            ],
        );
    }
}
