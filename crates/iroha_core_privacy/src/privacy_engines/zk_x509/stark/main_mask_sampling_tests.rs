//! Original-entropy parity, work bounds and real-cell clearing for mask sampling.

use super::*;
use rand::{RngCore, SeedableRng, rngs::StdRng};

fn native(column: usize, pattern: usize) -> ZeroizingMainTraceColumnV1 {
    ZeroizingMainTraceColumnV1(
        (0..8)
            .map(|row| match pattern {
                0 => F::ZERO,
                1 => F(u64::try_from((row + column) % 2).unwrap()),
                _ => F(u64::try_from(17 + 31 * column + 7 * row).unwrap()),
            })
            .collect(),
    )
}

#[test]
fn batched_mask_sampling_preserves_every_draw_and_masked_column() {
    for pattern in 0..3 {
        for width in [1, 7, 8, 9, 17] {
            let mut scalar_rng = StdRng::from_seed([91; 32]);
            let mut batch_rng = scalar_rng.clone();
            let scalar = MainTraceMaskGroupV1::sample_v1(3, 13, width, &mut scalar_rng, |column| {
                Ok(native(column, pattern))
            })
            .unwrap();
            let mut requests = Vec::new();
            let batch =
                MainTraceMaskGroupV1::sample_batched_v1(3, 13, width, &mut batch_rng, |columns| {
                    requests.push(columns.clone());
                    Ok(columns.map(|column| native(column, pattern)).collect())
                })
                .unwrap();
            assert_eq!(requests.len(), width.div_ceil(8));
            assert!(requests.iter().all(|range| range.len() <= 8));
            assert_eq!(
                requests.iter().cloned().flatten().collect::<Vec<_>>(),
                (0..width).collect::<Vec<_>>()
            );
            for column in 0..width {
                assert_eq!(
                    scalar.masks[column].coefficients(),
                    batch.masks[column].coefficients()
                );
                assert_eq!(
                    scalar.replay_v1(column, &native(column, pattern)).unwrap(),
                    batch.replay_v1(column, &native(column, pattern)).unwrap(),
                );
            }
            assert_eq!(scalar_rng.next_u64(), batch_rng.next_u64());
        }
    }
}

#[test]
fn malformed_sampling_batches_preserve_preflight_and_entropy_position() {
    for width in [0, usize::from(u16::MAX) + 1] {
        let mut rng = StdRng::from_seed([13; 32]);
        let mut unchanged = rng.clone();
        let mut calls = 0;
        assert!(
            MainTraceMaskGroupV1::sample_batched_v1(3, 13, width, &mut rng, |_| {
                calls += 1;
                Ok(Vec::new())
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert_eq!(rng.next_u64(), unchanged.next_u64());
    }
    for returned in [0, 7, 9] {
        let mut rng = StdRng::from_seed([17; 32]);
        let mut unchanged = rng.clone();
        assert!(
            MainTraceMaskGroupV1::sample_batched_v1(3, 13, 8, &mut rng, |_| {
                Ok((0..returned).map(|column| native(column, 2)).collect())
            })
            .is_err()
        );
        assert_eq!(rng.next_u64(), unchanged.next_u64());
    }
    let mut scalar_rng = StdRng::from_seed([19; 32]);
    let mut batch_rng = scalar_rng.clone();
    let column = |index| {
        let mut value = native(index, 2);
        if index == 3 {
            value[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        }
        value
    };
    assert!(
        MainTraceMaskGroupV1::sample_v1(3, 13, 8, &mut scalar_rng, |index| Ok(column(index)))
            .is_err()
    );
    assert!(
        MainTraceMaskGroupV1::sample_batched_v1(3, 13, 8, &mut batch_rng, |range| {
            Ok(range.map(column).collect())
        })
        .is_err()
    );
    assert_eq!(scalar_rng.next_u64(), batch_rng.next_u64());
}

struct PartialEntropy {
    draws: usize,
    panic_on_failure: bool,
}
impl TryRngCore for PartialEntropy {
    type Error = std::io::Error;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        unreachable!("mask sampler requests byte arrays")
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        unreachable!("mask sampler requests byte arrays")
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        assert_eq!(destination.len(), 8);
        if self.draws == 2 * (MASK_DEGREE + 1) + 2 {
            destination.fill(0x5a);
            assert!(!self.panic_on_failure, "injected entropy unwind");
            return Err(std::io::Error::other("injected entropy failure"));
        }
        destination.copy_from_slice(&(17 + u64::try_from(self.draws).unwrap()).to_le_bytes());
        self.draws += 1;
        Ok(())
    }
}

#[test]
fn pending_sampling_columns_clear_after_success_entropy_error_and_unwind() {
    use super::super::super::super::private_table::inspection;
    let assert_cleared = |observations: Vec<inspection::ErasureObservationV1>, expected: usize| {
        assert_eq!(
            observations.iter().map(|row| row.cells).sum::<usize>(),
            expected
        );
        assert_eq!(
            observations
                .iter()
                .map(|row| row.nonzero_before)
                .sum::<usize>(),
            expected
        );
        assert!(observations.iter().all(|row| row.nonzero_after == 0));
    };
    let (sampled, observations) = inspection::observe_v1(|| {
        MainTraceMaskGroupV1::sample_batched_v1(
            3,
            13,
            17,
            &mut StdRng::from_seed([23; 32]),
            |range| Ok(range.map(|column| native(column, 2)).collect()),
        )
    });
    assert!(sampled.is_ok());
    assert_cleared(observations, 17 * 8);
    for panic_on_failure in [false, true] {
        let mut rng = PartialEntropy {
            draws: 0,
            panic_on_failure,
        };
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                MainTraceMaskGroupV1::sample_batched_v1(3, 13, 17, &mut rng, |range| {
                    Ok(range.map(|column| native(column, 2)).collect())
                })
            }))
        });
        assert!(match result {
            Ok(result) => result.is_err(),
            Err(_) => panic_on_failure,
        });
        assert_cleared(observations, 8 * 8);
        assert_eq!(rng.draws, 2 * (MASK_DEGREE + 1) + 2);
    }
    for unwind in [false, true] {
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                MainTraceMaskGroupV1::sample_batched_v1(
                    3,
                    13,
                    17,
                    &mut StdRng::from_seed([29; 32]),
                    |range| {
                        let mut batch = Vec::with_capacity(range.len());
                        for column in range {
                            batch.push(native(column, 2));
                            if column == 10 {
                                assert!(!unwind, "injected partial source unwind");
                                return Err(ZkX509StarkErrorV1::InternalInvariant);
                            }
                        }
                        Ok(batch)
                    },
                )
            })
        });
        assert!(match result {
            Ok(result) => result.is_err(),
            Err(_) => unwind,
        });
        assert_cleared(observations, 11 * 8);
    }
}

#[test]
fn canonical_sha_sampling_work_is_batched_inside_existing_replay_allowance() {
    assert_eq!(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1, 8);
    assert_eq!(main_diagnostic_transform_columns_v1().unwrap(), 11_622);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let native = 1_usize << layout.trace_groups[5].native_trace_log2;
    let resident = 8
        * (native * core::mem::size_of::<F>() + core::mem::size_of::<ZeroizingMainTraceColumnV1>());
    assert!(resident < plan.replay_batch);
    let mut scalar_calls = 0;
    let mut batch_calls = 0;
    for kind in [MainTraceColumnKindV1::Base, MainTraceColumnKindV1::Aux] {
        let group = &layout.trace_groups[5];
        let width = match kind {
            MainTraceColumnKindV1::Base => group.base_width,
            MainTraceColumnKindV1::Aux => group.aux_width,
        };
        for registration in &layout.registered_segments {
            if registration.trace_group != 5
                || registration.segment.adapter != SegmentAdapterIdV1::Sha256CallBus
            {
                continue;
            }
            let (start, end) = match kind {
                MainTraceColumnKindV1::Base => {
                    (registration.base_start, registration.base_end().unwrap())
                }
                MainTraceColumnKindV1::Aux => {
                    (registration.aux_start, registration.aux_end().unwrap())
                }
            };
            scalar_calls += end - start;
            for first in (0..width).step_by(8) {
                let last = (first + 8).min(width);
                if first < end && last > start {
                    batch_calls += 1;
                }
            }
        }
    }
    assert_eq!(
        scalar_calls,
        4 * (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 + ZK_X509_SHA_BATCH_AUX_WIDTH_V1)
    );
    assert!(batch_calls * 7 < scalar_calls);
    assert!(batch_calls <= scalar_calls.div_ceil(8) + 8);
    eprintln!(
        "SHA sampling source constructions: {scalar_calls} -> {batch_calls}; native batch payload={resident}"
    );
}

#[test]
fn canonical_arithmetic_auxiliary_runs_stay_within_existing_native_batch() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let bound = aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1;
    let group = &layout.trace_groups[5];
    let resident = bound
        * ((1 << group.native_trace_log2) * core::mem::size_of::<F>()
            + core::mem::size_of::<ZeroizingMainTraceColumnV1>());
    assert!(resident < plan.replay_batch);
    let mut scalar = 0;
    let mut batched = 0;
    for registration in &layout.registered_segments {
        if registration.segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
            continue;
        }
        assert_eq!(registration.trace_group, 5);
        assert_eq!(registration.segment.aux_width, 72);
        let end = registration.aux_end().unwrap();
        scalar += registration.segment.aux_width;
        for start in (0..group.aux_width).step_by(bound) {
            if start < end && start + bound > registration.aux_start {
                batched += 1;
            }
        }
    }
    assert_eq!(scalar, 5 * 72);
    assert!(batched <= scalar.div_ceil(bound) + 5);
    assert!(batched * 7 < scalar);
    eprintln!(
        "arithmetic auxiliary full native traversals per replay: {scalar} -> {batched}; retained terminal delta=0; native batch payload={resident}"
    );
}

#[test]
#[ignore = "actual maximum bound sources, native dispatch and log22 commitment/opening parity; optimized qualification"]
fn actual_arithmetic_auxiliary_dispatch_preserves_seeded_commitments_deep_and_openings() {
    actual_auxiliary_dispatch_case_v1(SegmentAdapterIdV1::P256Arithmetic);
}

#[test]
#[ignore = "actual maximum value auxiliary dispatch, seeded commitment/DEEP/opening parity"]
fn actual_value_auxiliary_dispatch_preserves_seeded_commitments_deep_and_openings() {
    actual_auxiliary_dispatch_case_v1(SegmentAdapterIdV1::P256ValueBus);
}

fn actual_auxiliary_dispatch_case_v1(adapter: SegmentAdapterIdV1) {
    use crate::privacy_engines::zk_x509::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x81; 32],
            assembly.verifier_profile.compiled_profile_digest,
            core::array::from_fn(|index| digest(index as u8 + 1)),
        ),
        digest(0x91),
        digest(0xa1),
        digest(0xb1),
    )
    .unwrap();
    let sha =
        main_log19_sha_base_sources_v1(&assembly.sha_schedule, &assembly.sha_witnesses).unwrap();
    let p256 = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let bound = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
        &layout, &assembly, sha, p256, binding,
    )
    .unwrap();
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(
        &layout,
        &fixture.statement,
        &assembly.projection_trace,
    )
    .unwrap();
    let mut io =
        MainIoTraceGroupSourceV1::for_main_v1(&layout, &fixture.statement, &assembly.io).unwrap();
    projection
        .bind_challenges_v1(binding.main_post_base())
        .unwrap();
    io.bind_challenges_v1(binding.main_post_base()).unwrap();
    let sources = MainTraceReplaySourcesV1::Bound {
        log19: &bound,
        projection: &projection,
        io: &io,
    };
    let first_registration = layout
        .registered_segments
        .iter()
        .find(|registration| {
            registration.segment.adapter == adapter && registration.trace_group == 5
        })
        .copied()
        .unwrap();
    let boundary = first_registration.aux_end().unwrap();
    // Split one requested batch across two canonical registrations (including execution to sorted).
    let crossed = sources
        .native_columns_v1(
            &layout,
            MainTraceColumnKindV1::Aux,
            5,
            boundary - 3..boundary + 5,
        )
        .unwrap();
    for (offset, actual) in crossed.iter().enumerate() {
        let (registration, local) = registered_main_group_column_v1(
            &layout,
            5,
            MainTraceColumnKindV1::Aux,
            boundary - 3 + offset,
        )
        .unwrap();
        assert_eq!(
            **actual,
            *bound.native_aux_column_v1(registration, local).unwrap()
        );
    }
    drop(crossed);
    for range in [0..0, 0..9, usize::MAX..usize::MAX] {
        assert!(
            sources
                .native_columns_v1(&layout, MainTraceColumnKindV1::Aux, 5, range)
                .is_err()
        );
    }
    // Use two adjacent real auxiliary columns on the unchanged native19/common22
    // domains. These exact seeded masks feed real Merkle roots and opening replay.
    let start = first_registration.aux_start + 47;
    let width = 2;
    let mut scalar = |column| {
        let (registration, local) =
            registered_main_group_column_v1(&layout, 5, MainTraceColumnKindV1::Aux, start + column)
                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
        bound
            .native_aux_column_v1(registration, local)
            .map(ZeroizingMainTraceColumnV1::into_vec_v1)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
    };
    let sources_ref = &sources;
    let layout_ref = &layout;
    let batched = || {
        let mut pending = Vec::new().into_iter();
        move |column| {
            if pending.len() == 0 {
                assert_eq!(column, 0);
                pending = sources_ref
                    .native_columns_v1(
                        layout_ref,
                        MainTraceColumnKindV1::Aux,
                        5,
                        start..start + width,
                    )
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
                    .into_iter();
            }
            pending
                .next()
                .map(ZeroizingMainTraceColumnV1::into_vec_v1)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)
        }
    };
    let mut scalar_rng = StdRng::from_seed([53; 32]);
    let mut batch_rng = scalar_rng.clone();
    let indices = [0, 1, (1 << layout.common_lde_log2) - 1];
    let commit =
        |rng: &mut StdRng,
         source: &mut dyn FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>| {
            aggregate::commit_masked_trace_columns_v1(
                ZK_X509_DIGEST_CONTEXT_V1,
                AUX_LEAF_DOMAIN,
                AUX_NODE_DOMAIN,
                5,
                first_registration.segment.trace_log2,
                layout.common_lde_log2,
                width,
                MASK_DEGREE,
                &indices,
                rng,
                source,
            )
            .unwrap()
        };
    let (scalar_commitment, scalar_masks) = commit(&mut scalar_rng, &mut scalar);
    let (batch_commitment, batch_masks) = commit(&mut batch_rng, &mut batched());
    assert_eq!(scalar_rng.next_u64(), batch_rng.next_u64());
    assert_eq!(scalar_commitment, batch_commitment);
    let point = E::from_coefficients([F(113), F(127), F(131), F(137)]).unwrap();
    assert_eq!(
        aggregate::evaluate_masked_native_columns_at_deep_v1(&scalar_masks, point, scalar).unwrap(),
        aggregate::evaluate_masked_native_columns_at_deep_v1(&batch_masks, point, batched())
            .unwrap()
    );
    let opening_replay = aggregate::replay_masked_trace_columns_v1(
        ZK_X509_DIGEST_CONTEXT_V1,
        AUX_LEAF_DOMAIN,
        AUX_NODE_DOMAIN,
        5,
        &batch_masks,
        &indices,
        batched(),
    )
    .unwrap();
    assert_eq!(opening_replay, scalar_commitment);
}

#[test]
fn canonical_value_auxiliary_runs_stay_within_existing_native_batch() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let bound = aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1;
    let group = &layout.trace_groups[5];
    let resident = bound
        * ((1 << group.native_trace_log2) * core::mem::size_of::<F>()
            + core::mem::size_of::<ZeroizingMainTraceColumnV1>());
    assert!(resident < plan.replay_batch);
    let mut scalar = 0;
    let mut batched = 0;
    let mut endpoints = [0, 0];
    for registration in &layout.registered_segments {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || registration.trace_group != 5
        {
            continue;
        }
        let (_, local) = p256_instance_parts_v1(registration.segment.instance).unwrap();
        assert!(local <= 1);
        endpoints[usize::from(local)] += 1;
        let width = if local == 0 { 116 } else { 12 };
        assert_eq!(registration.segment.aux_width, width);
        let end = registration.aux_end().unwrap();
        scalar += width;
        for first in (0..group.aux_width).step_by(bound) {
            if first < end && first + bound > registration.aux_start {
                batched += 1;
            }
        }
    }
    assert_eq!(endpoints, [5, 5]);
    assert_eq!(scalar, 640);
    assert!(batched <= scalar.div_ceil(bound) + 10);
    assert!(batched * 7 < scalar);
    eprintln!(
        "value auxiliary source traversals per complete replay: {scalar} -> {batched}; native batch payload={resident}; no retained matrix delta"
    );
}

#[test]
fn canonical_p256_base_runs_preserve_registration_boundaries_and_batch_allowance() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let group = &layout.trace_groups[5];
    let mut rows_before = 0;
    let mut rows_after = 0;
    let mut columns = 0;
    let mut registrations = 0;
    for registration in &layout.registered_segments {
        if registration.trace_group != 5
            || !(registration.segment.adapter == SegmentAdapterIdV1::P256Arithmetic
                || (registration.segment.adapter == SegmentAdapterIdV1::P256ValueBus
                    && p256_instance_parts_v1(registration.segment.instance)
                        .is_some_and(|(_, local)| local <= 1)))
        {
            continue;
        }
        registrations += 1;
        let start = registration.base_start;
        let end = registration.base_end().unwrap();
        let mut seen = Vec::new();
        columns += end - start;
        rows_before += (end - start) * registration.segment.trace_size();
        for batch in (0..group.base_width).step_by(8) {
            let first = batch.max(start);
            let last = (batch + 8).min(end).min(group.base_width);
            if first >= last {
                continue;
            }
            assert!(last - first <= 8);
            for column in first..last {
                let (actual, local) = registered_main_group_column_v1(
                    &layout,
                    5,
                    MainTraceColumnKindV1::Base,
                    column,
                )
                .unwrap();
                assert_eq!(actual, *registration);
                assert_eq!(local, column - start);
                seen.push(column);
            }
            rows_after += registration.segment.trace_size();
        }
        assert_eq!(seen, (start..end).collect::<Vec<_>>());
    }
    assert_eq!(registrations, 15);
    assert_eq!(columns, 1_395);
    assert_eq!(rows_before, 1_395 * (1 << 19));
    assert!(rows_after <= 200 * (1 << 19));
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    assert!(
        8 * ((1 << 19) * core::mem::size_of::<F>()
            + core::mem::size_of::<ZeroizingMainTraceColumnV1>())
            < plan.replay_batch
    );
}
