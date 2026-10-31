// Exact complete-AIR controls for immutable validated composition capabilities.

#[test]
fn validated_sha_context_preserves_all_residues_segments_and_row_refusals() {
    use super::super::{
        credential_pre_aux::ValidatedMainCompositionChallengesV1,
        sha_word_stark::wide_fold::ShaWordFoldPowersV1,
    };
    let phase = credential_binding(0x49).main_post_base();
    let capability = ValidatedMainCompositionChallengesV1::new_v1(&phase).unwrap();
    let powers = ShaWordFoldPowersV1::new_v1(phase.sha_word()).unwrap();
    for seed in 0..4_u64 {
        let mut current = ZkX509ShaBatchRowV1 {
            base: core::array::from_fn(|i| {
                F::reduce((i as u128 + 1) * (GOLDILOCKS_MODULUS_V1 - 1 - seed) as u128)
            }),
            aux: core::array::from_fn(|i| F(i as u64 + seed + 17)),
            fixed: core::array::from_fn(|i| F(i as u64 + seed + 31)),
        };
        let mut next = current;
        for segment in 0..4 {
            for full in [false, true] {
                let terminals = neutral_ca_call_boundaries();
                let boundaries = full.then_some(&terminals);
                let expected = evaluate_sha_batch_relation_v1(
                    &current,
                    &next,
                    phase.sha_word(),
                    phase.sha(),
                    phase.rfc5280(),
                    segment,
                    boundaries,
                )
                .unwrap();
                let mut residues = vec![F(91); expected.len()];
                let allocation = (residues.as_ptr(), residues.capacity());
                residues.clear();
                evaluate_sha_batch_relation_with_context_v1(
                    &current,
                    &next,
                    ShaBatchEvaluationChallengesV1::Bound(capability),
                    segment,
                    boundaries,
                    &mut residues,
                    powers.row_v1(),
                )
                .unwrap();
                assert_eq!(residues, expected);
                assert_eq!((residues.as_ptr(), residues.capacity()), allocation);
                let mut streamed = FixedResidues::new();
                evaluate_sha_batch_relation_with_context_v1(
                    &current,
                    &next,
                    ShaBatchEvaluationChallengesV1::Bound(capability),
                    segment,
                    boundaries,
                    &mut streamed,
                    powers.row_v1(),
                )
                .unwrap();
                assert_eq!(streamed.values(), expected);
                assert_eq!(streamed.reserves, [155]);
                if !full {
                    residues.clear();
                    evaluate_sha_batch_local_with_bound_context_into_v1(
                        &current,
                        &next,
                        capability,
                        segment,
                        &mut residues,
                        &powers,
                    )
                    .unwrap();
                    assert_eq!(residues, expected);
                    let mut streamed = FixedResidues::new();
                    evaluate_sha_batch_local_with_bound_context_into_v1(
                        &current,
                        &next,
                        capability,
                        segment,
                        &mut streamed,
                        &powers,
                    )
                    .unwrap();
                    assert_eq!(streamed.values(), expected);
                }
            }
        }
        for fault in 0..8 {
            let original = (current, next);
            let mut segment = 0;
            let mut terminals = neutral_ca_call_boundaries();
            match fault {
                0 => current.base[0] = F(GOLDILOCKS_MODULUS_V1),
                1 => current.aux[0] = F(GOLDILOCKS_MODULUS_V1),
                2 => current.fixed[0] = F(GOLDILOCKS_MODULUS_V1),
                3 => next.base[0] = F(GOLDILOCKS_MODULUS_V1),
                4 => next.aux[0] = F(GOLDILOCKS_MODULUS_V1),
                5 => next.fixed[0] = F(GOLDILOCKS_MODULUS_V1),
                6 => segment = 255,
                _ => terminals[0].call = 255,
            }
            let expected = evaluate_sha_batch_relation_v1(
                &current,
                &next,
                phase.sha_word(),
                phase.sha(),
                phase.rfc5280(),
                segment,
                Some(&terminals),
            )
            .unwrap_err();
            let mut residues = vec![F(91); ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1];
            let before = residues.clone();
            let actual = evaluate_sha_batch_relation_with_context_v1(
                &current,
                &next,
                ShaBatchEvaluationChallengesV1::Bound(capability),
                segment,
                Some(&terminals),
                &mut residues,
                powers.row_v1(),
            )
            .unwrap_err();
            assert_eq!(actual, expected);
            assert_eq!(residues, before);
            (current, next) = original;
        }
    }
    let other_phase = credential_binding(0x4a).main_post_base();
    let other_powers = ShaWordFoldPowersV1::new_v1(other_phase.sha_word()).unwrap();
    let row = ZkX509ShaBatchRowV1 {
        base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
        aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
        fixed: [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    };
    let mut residues = vec![F(97); ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1];
    let before = residues.clone();
    assert!(
        evaluate_sha_batch_local_with_bound_context_into_v1(
            &row,
            &row,
            capability,
            0,
            &mut residues,
            &other_powers
        )
        .is_err()
    );
    assert_eq!(residues, before);
}

#[test]
fn validated_rfc_context_preserves_every_local_residue_and_row_refusal() {
    use super::super::{
        credential_pre_aux::ValidatedMainCompositionChallengesV1, rfc5280_stark::*,
    };
    let phase = credential_binding(0x4b).main_post_base();
    let capability = ValidatedMainCompositionChallengesV1::new_v1(&phase).unwrap();
    for seed in 0..4_u64 {
        let mut current: ZkX509Rfc5280StarkBaseRowV1 =
            core::array::from_fn(|i| F(i as u64 + seed + 1));
        let mut next = current;
        let mut aux: ZkX509Rfc5280StarkAuxRowV1 = core::array::from_fn(|i| F(i as u64 + seed + 17));
        let mut next_aux = aux;
        let mut fixed: ZkX509Rfc5280StarkFixedRowV1 =
            core::array::from_fn(|i| F(i as u64 + seed + 31));
        for fault in 0..6 {
            let original = (current, next, aux, next_aux, fixed);
            match fault {
                1 => current[0] = F(GOLDILOCKS_MODULUS_V1),
                2 => next[0] = F(GOLDILOCKS_MODULUS_V1),
                3 => aux[0] = F(GOLDILOCKS_MODULUS_V1),
                4 => next_aux[0] = F(GOLDILOCKS_MODULUS_V1),
                5 => fixed[0] = F(GOLDILOCKS_MODULUS_V1),
                _ => (),
            }
            let expected = evaluate_zk_x509_rfc5280_local_residues_v1(
                &current,
                &next,
                &aux,
                &next_aux,
                &fixed,
                &phase.der(),
                phase.rfc5280(),
            );
            let actual = evaluate_rfc5280_local_with_bound_context_v1(
                &current, &next, &aux, &next_aux, &fixed, capability,
            );
            assert_eq!(actual, expected);
            if let Ok(residues) = actual {
                assert_eq!(residues.len(), ZK_X509_RFC5280_LOCAL_CONSTRAINT_COUNT_V1);
            }
            (current, next, aux, next_aux, fixed) = original;
        }
    }
}

#[test]
#[ignore = "full SHA/RFC AIR raw-validation versus immutable-capability cost"]
fn validated_composition_context_cost_preserves_complete_air_outputs() {
    use super::super::{
        credential_pre_aux::ValidatedMainCompositionChallengesV1, rfc5280_stark::*,
        sha_word_stark::wide_fold::ShaWordFoldPowersV1,
    };
    use rayon::prelude::*;
    use std::{hint::black_box, time::Instant};
    assert!(!cfg!(debug_assertions), "use an optimized build");
    let phase = credential_binding(0x4c).main_post_base();
    let setup = Instant::now();
    let capability = ValidatedMainCompositionChallengesV1::new_v1(&phase).unwrap();
    let setup_ns = setup.elapsed().as_nanos();
    let powers = ShaWordFoldPowersV1::new_v1(phase.sha_word()).unwrap();
    let sha = ZkX509ShaBatchRowV1 {
        base: core::array::from_fn(|i| F(i as u64 + 1)),
        aux: core::array::from_fn(|i| F(i as u64 + 17)),
        fixed: core::array::from_fn(|i| F(i as u64 + 31)),
    };
    let rfc: ZkX509Rfc5280StarkBaseRowV1 = core::array::from_fn(|i| F(i as u64 + 1));
    let rfc_aux: ZkX509Rfc5280StarkAuxRowV1 = core::array::from_fn(|i| F(i as u64 + 17));
    let rfc_fixed: ZkX509Rfc5280StarkFixedRowV1 = core::array::from_fn(|i| F(i as u64 + 31));
    let expected_sha = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &sha,
        &sha,
        phase.sha_word(),
        phase.sha(),
        phase.rfc5280(),
        0,
    )
    .unwrap();
    let expected_rfc = evaluate_zk_x509_rfc5280_local_residues_v1(
        &rfc,
        &rfc,
        &rfc_aux,
        &rfc_aux,
        &rfc_fixed,
        &phase.der(),
        phase.rfc5280(),
    )
    .unwrap();
    const ROWS: usize = 1 << 15;
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|_| ());
        let mut buffers = PrivateTableV1::new(
            (0..workers)
                .map(|_| vec![F::ZERO; ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1])
                .collect(),
            zeroize_field_rows_v1::<Vec<F>>,
        );
        for rfc_mode in [false, true] {
            for round in 0..4 {
                for bound in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let started = Instant::now();
                    pool.install(|| {
                        buffers
                            .par_iter_mut()
                            .enumerate()
                            .for_each(|(worker, residues)| {
                                for _ in (worker..ROWS).step_by(workers) {
                                    if rfc_mode {
                                        let actual = if bound {
                                            evaluate_rfc5280_local_with_bound_context_v1(
                                                black_box(&rfc),
                                                &rfc,
                                                &rfc_aux,
                                                &rfc_aux,
                                                &rfc_fixed,
                                                capability,
                                            )
                                        } else {
                                            evaluate_zk_x509_rfc5280_local_residues_v1(
                                                black_box(&rfc),
                                                &rfc,
                                                &rfc_aux,
                                                &rfc_aux,
                                                &rfc_fixed,
                                                &phase.der(),
                                                phase.rfc5280(),
                                            )
                                        }
                                        .unwrap();
                                        black_box(PrivateTableV1::new(
                                            actual,
                                            super::super::private_table::zeroize_fields_v1,
                                        ));
                                    } else {
                                        if bound {
                                            residues.clear();
                                            evaluate_sha_batch_local_with_bound_context_into_v1(
                                                black_box(&sha),
                                                &sha,
                                                capability,
                                                0,
                                                residues,
                                                &powers,
                                            )
                                            .unwrap();
                                        } else {
                                            residues.clear();
                                            evaluate_sha_batch_local_with_powers_into_v1(
                                                black_box(&sha),
                                                &sha,
                                                phase.sha_word(),
                                                phase.sha(),
                                                phase.rfc5280(),
                                                0,
                                                residues,
                                                &powers,
                                            )
                                            .unwrap();
                                        }
                                        black_box(&*residues);
                                    }
                                }
                            })
                    });
                    let seconds = started.elapsed().as_secs_f64();
                    if !rfc_mode {
                        for residues in buffers.iter() {
                            assert_eq!(residues, &expected_sha);
                        }
                    }
                    let final_rfc = evaluate_rfc5280_local_with_bound_context_v1(
                        &rfc, &rfc, &rfc_aux, &rfc_aux, &rfc_fixed, capability,
                    )
                    .unwrap();
                    assert_eq!(final_rfc, expected_rfc);
                    eprintln!(
                        "X509 complete AIR context A/B: rfc={rfc_mode} workers={workers} round={round} bound={bound} rows={ROWS} seconds={seconds:.9} setup_ns={setup_ns} capability_bytes={} parity=all_fields_exact; original row-dependent checks retained; SHA uses same wide fold in both arms; RFC allocation and clearing included in both arms",
                        core::mem::size_of_val(&capability)
                    );
                }
            }
        }
    }
}
