//! Whole-profile retained allocation bounds, without materializing private traces.

use super::*;

#[test]
fn small_native_source_forecast_uses_full_widths_and_rejects_oversized_extents() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let shape = main_resources::MainSmallSourceShapeV1 {
        der_active_rows: ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        io_active_rows: 1 << 18,
        io_declarations_payload: 4096,
        projection_rows: 1 << 15,
    };
    let (retained, scratch) = shape.allocation_forecast_v1(&layout).unwrap();
    let matrices = 327_680 * (76 + 196) * 8 + (1 << 18) * (28 + 17 + 39) * 8 + (1 << 15) * 32 * 8;
    assert_eq!(matrices, 897_581_056);
    assert!(retained >= matrices + 4096);
    assert!(scratch < main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
    let mut changed = shape;
    changed.der_active_rows += 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_active_rows += 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.projection_rows -= 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_declarations_payload = usize::MAX;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_active_rows = 1;
    assert_eq!(
        changed.allocation_forecast_v1(&layout).unwrap(),
        (retained, scratch)
    );
}

#[test]
fn canonical_source_shape_forecasts_fit_the_preconstruction_allowances() {
    use super::super::super::sha_call_bus_stark::ZkX509ShaCallPublicShapeV1;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let (small, small_scratch) = main_resources::MainSmallSourceShapeV1 {
        der_active_rows: ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        io_active_rows: 1 << 18,
        io_declarations_payload: 1 << 20,
        projection_rows: 1 << 15,
    }
    .allocation_forecast_v1(&layout)
    .unwrap();
    let sha_shape = ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    };
    let p256 = P256MainBaseSourceV1::allocation_forecast_v1().unwrap();
    let p256_scratch = P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap();
    let sha_fixed = ZkX509ShaBatchFixedProviderV1::allocation_forecast_v1(sha_shape).unwrap();
    let sha_scratch = ZkX509ShaBatchFixedProviderV1::replay_scratch_forecast_v1(sha_shape).unwrap();
    let retained = small + p256 + sha_fixed;
    assert!(retained <= main_resources::MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1);
    let serial_scratch = small_scratch.max(sha_scratch).max(p256_scratch);
    assert!(serial_scratch <= main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
    eprintln!(
        "MAIN source shape forecasts: retained={retained}, small={small}, p256={p256}, sha_fixed={sha_fixed}, serial_scratch={}",
        serial_scratch
    );
    // These construction bounds do not materialize or qualify the separately
    // borrowed maximum-profile assembly, nor measure allocator/process RSS.
}

#[test]
fn source_payload_admission_includes_scratch_reserve_and_capacity_overflow() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let reserve = main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1;
    let scratch = 13_579;
    let available = plan.remaining_source_and_runtime_envelope - reserve - scratch;
    let first = available / 3;
    let second = available - first;
    assert_eq!(
        plan.check_source_payloads_v1(&[first, second], scratch)
            .unwrap() as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(matches!(
        plan.check_source_payloads_v1(&[first, second + 1], scratch),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(matches!(
        plan.check_source_payloads_v1(&[first, second], scratch + 1),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(plan.check_source_payloads_v1(&[usize::MAX, 1], 0).is_err());
    assert!(plan.check_source_payloads_v1(&[0], usize::MAX).is_err());
    assert_eq!(
        plan.check_source_payloads_v1(&[], 0).unwrap(),
        plan.maximum_live_buffers + reserve
    );
}

#[test]
fn native_source_budget_is_reserved_before_construction_and_rechecked_after_binding() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let source_limit = main_resources::MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1;
    let scratch = main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1;
    let reserve = main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1;
    let assembly_limit =
        plan.remaining_source_and_runtime_envelope - source_limit - scratch - reserve;
    assert_eq!(
        plan.check_before_sources_v1(assembly_limit).unwrap() as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(matches!(
        plan.check_before_sources_v1(assembly_limit + 1),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert_eq!(
        plan.check_native_sources_v1(assembly_limit, &[source_limit / 2, source_limit / 2])
            .unwrap(),
        plan.check_before_sources_v1(assembly_limit).unwrap()
    );
    assert!(matches!(
        plan.check_native_sources_v1(0, &[source_limit, 1]),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(plan.check_native_sources_v1(0, &[usize::MAX, 1]).is_err());
    // The source transition reuses the same retained assembly charge, rather
    // than treating a borrowed assembly as already freed after base commitment.
    assert!(matches!(
        plan.check_native_sources_v1(assembly_limit + 1, &[source_limit]),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
}

#[test]
fn replay_buffer_plan_charges_live_owners_and_leaves_an_explicit_source_envelope() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(plan.masks, 81_690_944);
    assert_eq!(
        plan.joined_streams,
        layout.common_lde_size()
            * core::mem::size_of::<
                crate::privacy_engines::privacy_outer_hash::PrivacyOuterLastFieldStreamV1,
            >()
    );
    assert_eq!(
        plan.maximum_live_buffers,
        plan.masks
            + (plan.quotient_stage + plan.replay_batch).max(
                plan.joined_streams
                    + plan.replay_batch
                    + plan.composition
                    + plan.fri_stage
                    + plan.openings
            )
    );
    assert_eq!(
        (plan.maximum_live_buffers + plan.remaining_source_and_runtime_envelope) as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(plan.remaining_source_and_runtime_envelope > 0);
    // This remainder must also cover borrowed sources and process overhead;
    // passing this buffer check is not whole-prover or RSS qualification.
    eprintln!("MAIN replay transform buffer plan: {plan:?}");
    let mut invalid = layout;
    invalid.trace_groups[5].base_width += 1;
    assert!(main_resources::MainProverBufferPlanV1::new_v1(&invalid).is_err());
}

#[test]
fn whole_main_retaining_every_masked_coefficient_exceeds_the_release_memory_ceiling() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let geometry = layout
        .trace_groups
        .iter()
        .map(|group| (group.native_trace_log2, group.base_width + group.aux_width))
        .collect::<Vec<_>>();
    assert_eq!(
        geometry,
        [
            (5, 800),
            (8, 190),
            (15, 49),
            (16, 805),
            (18, 67),
            (19, 3712)
        ]
    );
    let masks = MASK_DEGREE + 1;
    assert_eq!(masks, 1816);
    let coefficient_bytes = geometry
        .iter()
        .map(|(log, width)| ((1_u64 << log) + masks as u64) * *width as u64 * 8)
        .sum::<u64>();
    let mask_bytes = geometry
        .iter()
        .map(|(_, width)| *width as u64 * masks as u64 * 8)
        .sum::<u64>();
    assert_eq!(coefficient_bytes, 16_226_947_392);
    assert_eq!(mask_bytes, 81_690_944);
    assert!(coefficient_bytes > super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1);
    // This is a payload lower bound, excluding borrowed assembly/source traces,
    // quotient matrices, public fixed polynomials, FFT scratch and allocator overhead.
    assert_eq!(((1_u64 << 19) + masks as u64) * 3712 * 8, 15_623_184_384);
}

/// Run in a separate optimized process with `/usr/bin/time -l` on macOS.
/// This constructs real maximum-structural-shape private inputs and the complete
/// assembly, but deliberately stops before mask sampling or proof construction.
#[test]
#[ignore = "native maximum-shape MAIN assembly allocation/time diagnostic; run --release"]
fn maximum_profile_assembly_payload_and_source_admission_diagnostic() {
    use super::super::super::{
        allocation_payload::{p256_material_v1, sum_v1, vector_v1},
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        profile::ZK_X509_PROVER_TARGET_SECONDS_V1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    use std::time::Instant;

    assert!(
        !cfg!(debug_assertions),
        "run this diagnostic with --release"
    );
    let fixture_start = Instant::now();
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum structural release fixture");
    fixture.resource_shape.validate_v1().unwrap();
    assert_eq!(fixture.resource_shape.certificate_chain_depth, 3);
    assert_eq!(fixture.statement.disclosed_attributes.len(), 4);
    assert_eq!(fixture.crl_entry_count, 64);
    assert_eq!(fixture.resource_shape.maximum_serial_bytes, 20);
    eprintln!(
        "X509 assembly diagnostic fixture: seconds={:.6}, shape={:?}",
        fixture_start.elapsed().as_secs_f64(),
        fixture.resource_shape,
    );
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let start = Instant::now();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .expect("complete maximum structural MAIN assembly");
    let construction = start.elapsed();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let payload = assembly.allocated_payload_bytes_v1();
    let p256_materials = sum_v1(assembly.p256_materials.iter().map(p256_material_v1));
    let der_rows = vector_v1(&assembly.der_base.rows);
    let io_rows = vector_v1(&assembly.io.execution) + vector_v1(&assembly.io.sorted);
    let projection_rows = vector_v1(&assembly.projection_trace.base.rows);
    let remaining_owners = payload
        .checked_sub(p256_materials + der_rows + io_rows + projection_rows)
        .expect("component allocation charges are disjoint");
    let assembly_allowance = plan
        .remaining_source_and_runtime_envelope
        .checked_sub(
            main_resources::MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1
                + main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
                + main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1,
        )
        .unwrap();
    let total_columns = layout
        .trace_groups
        .iter()
        .map(|group| group.base_width + group.aux_width)
        .sum::<usize>();
    let common_rows = 1_usize << layout.common_lde_log2;
    let commitment_field_bytes = 2_u64 * total_columns as u64 * common_rows as u64 * 8;
    eprintln!(
        "X509 assembly diagnostic payload: actual={payload}, allowance={assembly_allowance}, p256_materials={p256_materials}, der_rows={der_rows}, io_rows={io_rows}, projection_rows={projection_rows}, other_owned={remaining_owners}, construction_seconds={:.6}, total_prover_target_seconds={ZK_X509_PROVER_TARGET_SECONDS_V1}",
        construction.as_secs_f64(),
    );
    eprintln!(
        "X509 remaining work floor: trace_columns={total_columns}, common_rows={common_rows}, original_plus_opening_commitment_field_bytes={commitment_field_bytes}; excludes source replay, interpolation, quotient stripes, Fp4 constraints/DEEP, FRI, CA, framing and Merkle parents",
    );
    // Print measured data before rejecting the fixture. Do not relax the
    // production allowance merely to turn this diagnostic into a passing test.
    plan.check_source_shapes_v1(&layout, &assembly)
        .expect("maximum structural assembly fits unchanged preconstruction admission");
    let before_drop = Instant::now();
    drop(assembly);
    eprintln!(
        "X509 assembly diagnostic cleanup: seconds={:.6}; payload is not process RSS, construction-only timing is not full-proof qualification",
        before_drop.elapsed().as_secs_f64(),
    );
    assert!(
        construction.as_secs() < ZK_X509_PROVER_TARGET_SECONDS_V1,
        "assembly alone exhausts the unchanged total proving target"
    );
}

/// Synthetic public values isolate the real exact-width streaming hash work.
/// The small row sample fits caches differently from a full commitment, so its
/// linear extrapolation is diagnostic only and never qualifies the time limit.
#[test]
#[ignore = "optimized exact-width X509 streaming hash cost diagnostic; run --release"]
fn maximum_profile_streaming_hash_cost_diagnostic() {
    use std::time::Instant;

    assert!(
        !cfg!(debug_assertions),
        "run this diagnostic with --release"
    );
    const SAMPLE_ROWS: usize = 4096;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let widths = [
        layout
            .trace_groups
            .iter()
            .map(|group| group.base_width)
            .sum::<usize>(),
        layout
            .trace_groups
            .iter()
            .map(|group| group.aux_width)
            .sum::<usize>(),
    ];
    let full_rows = 1_usize << layout.common_lde_log2;
    let available = std::thread::available_parallelism().unwrap().get();
    let mut workers = vec![1, available];
    workers.dedup();
    let mut expected_roots = None;
    for workers in workers {
        for batch_width in [1, aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] {
            let (roots, seconds) = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let start = Instant::now();
                    let mut roots = Vec::new();
                    for (width, (leaf, node)) in widths.into_iter().zip([
                        (
                            AGGREGATE_DOMAINS_V1.base_leaf,
                            AGGREGATE_DOMAINS_V1.base_node,
                        ),
                        (AGGREGATE_DOMAINS_V1.aux_leaf, AGGREGATE_DOMAINS_V1.aux_node),
                    ]) {
                        let mut commitment = aggregate::StreamingRowCommitmentV1::new(
                            AGGREGATE_DOMAINS_V1.digest_context,
                            leaf,
                            node,
                            usize::from(u16::MAX),
                            SAMPLE_ROWS,
                            width,
                            &[],
                        )
                        .unwrap();
                        for first in (0..width).step_by(batch_width) {
                            let columns = (first..(first + batch_width).min(width))
                                .map(|column| {
                                    (0..SAMPLE_ROWS)
                                        .map(|row| F((row * 31 + column * 17) as u64))
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();
                            commitment.absorb_columns_v1(&columns).unwrap();
                        }
                        roots.push(commitment.finish().unwrap().commitment.root);
                    }
                    (roots, start.elapsed().as_secs_f64())
                });
            if let Some(expected) = &expected_roots {
                assert_eq!(&roots, expected);
            } else {
                expected_roots = Some(roots);
            }
            eprintln!(
                "X509 exact-width streaming hash diagnostic: workers={workers}, batch_columns={batch_width}, sample_rows={SAMPLE_ROWS}, base_width={}, aux_width={}, seconds={seconds:.6}, linear_two_full_pass_seconds={:.3}; excludes FFT/source replay/constraints/FRI/CA, differs in cache residency, concurrent host load must be recorded",
                widths[0],
                widths[1],
                seconds * 2.0 * (full_rows / SAMPLE_ROWS) as f64,
            );
        }
    }
}
